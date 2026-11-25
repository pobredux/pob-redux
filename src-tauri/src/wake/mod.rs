//! Wake word: listens for "Hey Redux", records the question after it, and fits it to the user's voice.

mod net;

use std::collections::VecDeque;
use std::path::PathBuf;
use std::sync::{mpsc, Arc, Mutex};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use rubato::audioadapter_buffers::direct::InterleavedSlice;
use rubato::{Fft, FixedSync, Resampler};
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, Manager, State};

use net::{fit_personal, Detector, Embedding, Head, HOP, RATE};

const PROFILE: &str = "wake-profile.json";
const PRE_ROLL: f64 = 1.5;
const GRACE: f64 = 0.6;
const WAIT_FOR_QUESTION: f64 = 5.0;
const END_SILENCE: f64 = 1.0;
const MAX_QUESTION: f64 = 15.0;
const AUTO_RESUME: f64 = 120.0;
const TRY_END_SILENCE: f64 = 0.6;
const TRY_LIMIT: f64 = 7.0;
const READ_END_SILENCE: f64 = 1.5;
const READ_LIMIT: f64 = 60.0;
const SILENT_TRIES: usize = 3;

enum Cmd {
    Stop,
    Resume,
    CancelCapture,
    SetupStart { tries: usize, read_seconds: f64 },
    SetupCancel,
    SetupNext,
    UseHead(Box<Head>),
}

struct Listener {
    tx: mpsc::Sender<Cmd>,
    join: std::thread::JoinHandle<()>,
}

#[derive(Default)]
pub struct WakeState {
    listener: Mutex<Option<Listener>>,
    captured: Arc<Mutex<Option<Vec<f32>>>>,
    pending: Arc<Mutex<Option<Head>>>,
}

#[derive(Serialize, Deserialize)]
struct Profile {
    #[serde(flatten)]
    head: Head,
    created: u64,
}

#[derive(Serialize)]
pub struct WakeStatus {
    running: bool,
    profile: Option<u64>,
}

#[derive(Serialize, Clone)]
#[serde(tag = "stage", rename_all = "lowercase")]
enum SetupEvent {
    Ask { attempt: usize, total: usize, test: bool },
    Result { attempt: usize, total: usize, test: bool, voice: bool, heard: bool, personal: Option<bool>, best: f32 },
    Read { needed: f64 },
    Reading { voiced: f64, needed: f64 },
    Fitted { min_positive: f32, max_negative: f32, threshold: f32, separated: bool },
    Done { generic: usize, personal: usize, tries: usize },
    Failed { error: String },
}

fn profile_path(app: &AppHandle) -> Result<PathBuf, String> {
    Ok(app.path().app_config_dir().map_err(|e| e.to_string())?.join(PROFILE))
}

fn load_profile(app: &AppHandle) -> Option<Profile> {
    serde_json::from_str(&std::fs::read_to_string(profile_path(app).ok()?).ok()?).ok()
}

impl WakeState {
    fn send(&self, cmd: Cmd) -> bool {
        let mut slot = self.listener.lock().unwrap();
        if slot.as_ref().is_some_and(|l| l.join.is_finished()) {
            *slot = None;
        }
        slot.as_ref().is_some_and(|l| l.tx.send(cmd).is_ok())
    }

    fn start(&self, app: &AppHandle) -> Result<(), String> {
        let mut slot = self.listener.lock().unwrap();
        if slot.as_ref().is_some_and(|l| !l.join.is_finished()) {
            return Ok(());
        }
        let (tx, rx) = mpsc::channel();
        let (ready_tx, ready) = mpsc::sync_channel(1);
        let (app2, captured, pending) = (app.clone(), self.captured.clone(), self.pending.clone());
        let join = std::thread::Builder::new()
            .name("wake-listen".into())
            .spawn(move || listen(app2, rx, ready_tx, captured, pending))
            .map_err(|e| e.to_string())?;
        match ready.recv() {
            Ok(Ok(())) => {
                *slot = Some(Listener { tx, join });
                Ok(())
            }
            Ok(Err(e)) => Err(e),
            Err(_) => Err("The wake word listener stopped unexpectedly.".into()),
        }
    }
}

#[tauri::command]
pub fn wake_status(app: AppHandle, state: State<'_, WakeState>) -> WakeStatus {
    let running = state.listener.lock().unwrap().as_ref().is_some_and(|l| !l.join.is_finished());
    WakeStatus { running, profile: load_profile(&app).map(|p| p.created) }
}

#[tauri::command]
pub fn wake_start(app: AppHandle, state: State<'_, WakeState>) -> Result<(), String> {
    state.start(&app)
}

#[tauri::command]
pub fn wake_stop(state: State<'_, WakeState>) {
    if let Some(l) = state.listener.lock().unwrap().take() {
        let _ = l.tx.send(Cmd::Stop);
        let _ = l.join.join();
    }
}

#[tauri::command]
pub fn wake_resume(state: State<'_, WakeState>) {
    state.send(Cmd::Resume);
}

#[tauri::command]
pub fn wake_cancel(state: State<'_, WakeState>) {
    state.send(Cmd::CancelCapture);
}

#[tauri::command]
pub fn wake_setup_start(app: AppHandle, state: State<'_, WakeState>, tries: usize, read_seconds: f64) -> Result<(), String> {
    state.start(&app)?;
    *state.pending.lock().unwrap() = None;
    state.send(Cmd::SetupStart { tries: tries.clamp(3, 20), read_seconds: read_seconds.clamp(6.0, 40.0) });
    Ok(())
}

#[tauri::command]
pub fn wake_setup_cancel(state: State<'_, WakeState>) {
    state.send(Cmd::SetupCancel);
}

#[tauri::command]
pub fn wake_setup_next(state: State<'_, WakeState>) {
    state.send(Cmd::SetupNext);
}

#[tauri::command]
pub fn wake_setup_save(app: AppHandle, state: State<'_, WakeState>) -> Result<(), String> {
    let head = state.pending.lock().unwrap().take().ok_or("There is no finished setup to save.")?;
    let path = profile_path(&app)?;
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    }
    let created = SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
    let json = serde_json::to_string(&Profile { head: head.clone(), created }).map_err(|e| e.to_string())?;
    std::fs::write(&path, json).map_err(|e| format!("{}: {e}", path.display()))?;
    state.send(Cmd::UseHead(Box::new(head)));
    Ok(())
}

#[tauri::command]
pub fn wake_profile_reset(app: AppHandle, state: State<'_, WakeState>) -> Result<(), String> {
    let path = profile_path(&app)?;
    match std::fs::remove_file(&path) {
        Ok(()) => {}
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
        Err(e) => return Err(format!("{}: {e}", path.display())),
    }
    state.send(Cmd::UseHead(Box::new(Head::generic())));
    Ok(())
}

#[tauri::command]
pub async fn wake_transcribe(app: AppHandle, state: State<'_, WakeState>, model: String, language: String, prompt: String) -> Result<String, String> {
    let audio = state.captured.lock().unwrap().take().ok_or("No question was recorded.")?;
    tauri::async_runtime::spawn_blocking(move || crate::voice::transcribe_clip(&app, &model, &language, &prompt, &audio))
        .await
        .map_err(|e| e.to_string())?
}

fn open_input(tx: mpsc::Sender<Vec<f32>>) -> Result<(cpal::Stream, u32), String> {
    let device = cpal::default_host().default_input_device().ok_or("No microphone was found.")?;
    let supported = device.default_input_config().map_err(|e| format!("The microphone could not be opened: {e}"))?;
    let rate = supported.sample_rate();
    let channels = supported.channels().max(1) as usize;
    let format = supported.sample_format();
    let config: cpal::StreamConfig = supported.into();
    fn build<T>(device: &cpal::Device, config: cpal::StreamConfig, channels: usize, tx: mpsc::Sender<Vec<f32>>) -> Result<cpal::Stream, cpal::Error>
    where
        T: cpal::SizedSample,
        f32: cpal::FromSample<T>,
    {
        device.build_input_stream(
            config,
            move |data: &[T], _: &cpal::InputCallbackInfo| {
                let _ = tx.send(data.chunks(channels).map(|f| f.iter().map(|x| x.to_sample::<f32>()).sum::<f32>() / f.len() as f32).collect());
            },
            |e| log::warn!("wake: microphone: {e}"),
            None,
        )
    }
    let stream = match format {
        cpal::SampleFormat::F32 => build::<f32>(&device, config, channels, tx),
        cpal::SampleFormat::I16 => build::<i16>(&device, config, channels, tx),
        cpal::SampleFormat::I32 => build::<i32>(&device, config, channels, tx),
        cpal::SampleFormat::U16 => build::<u16>(&device, config, channels, tx),
        cpal::SampleFormat::U8 => build::<u8>(&device, config, channels, tx),
        other => return Err(format!("The microphone uses a sample format this app cannot read ({other:?}).")),
    }
    .map_err(|e| format!("The microphone could not be opened: {e}"))?;
    stream.play().map_err(|e| format!("The microphone could not start: {e}"))?;
    Ok((stream, rate))
}

/// Dev hook (POB_REDUX_WAKE_WAV): a 16-bit PCM file in real time instead of the microphone, then silence.
fn play_file(path: &str, tx: mpsc::Sender<Vec<f32>>) -> Result<u32, String> {
    let b = std::fs::read(path).map_err(|e| format!("{path}: {e}"))?;
    let (mut rate, mut channels, mut pcm, mut at) = (16_000u32, 1usize, Vec::new(), 12);
    while at + 8 <= b.len() {
        let len = u32::from_le_bytes([b[at + 4], b[at + 5], b[at + 6], b[at + 7]]) as usize;
        let body = &b[at + 8..(at + 8 + len).min(b.len())];
        if &b[at..at + 4] == b"fmt " && body.len() >= 8 {
            channels = u16::from_le_bytes([body[2], body[3]]).max(1) as usize;
            rate = u32::from_le_bytes([body[4], body[5], body[6], body[7]]);
        } else if &b[at..at + 4] == b"data" {
            let s: Vec<f32> = body.as_chunks::<2>().0.iter().map(|x| i16::from_le_bytes(*x) as f32 / 32768.0).collect();
            pcm = s.chunks(channels).map(|f| f.iter().sum::<f32>() / f.len() as f32).collect();
        }
        at += 8 + len + (len & 1);
    }
    let step = (rate / 100) as usize;
    std::thread::spawn(move || {
        let mut chunks = pcm.chunks(step).map(<[f32]>::to_vec);
        loop {
            let chunk = chunks.next().unwrap_or_else(|| vec![0.0; step]);
            if tx.send(chunk).is_err() {
                return;
            }
            std::thread::sleep(Duration::from_millis(10));
        }
    });
    Ok(rate)
}

fn rms(x: &[f32]) -> f32 {
    (x.iter().map(|s| s * s).sum::<f32>() / x.len().max(1) as f32).sqrt()
}

enum Ask {
    Wait { until: f64, next: usize },
    Listen { attempt: usize, started: f64, end: Option<f64>, windows: Vec<(f32, Embedding)>, generic: bool, personal: bool, voiced: bool, quiet: f64 },
    Read { started: f64, voiced: f64, quiet: f64, shown: f64 },
}

struct Setup {
    tries: usize,
    read_seconds: f64,
    test: bool,
    ask: Ask,
    user_pos: Vec<Embedding>,
    best_per_try: Vec<Embedding>,
    user_neg: Vec<Embedding>,
    fitted: Option<Head>,
    heard: (usize, usize),
    floor: VecDeque<f32>,
    silent: usize,
    finish_read: bool,
}

impl Setup {
    /// Speech means clearly louder than the room was between prompts.
    fn speech(&self) -> f32 {
        let mut sorted: Vec<f32> = self.floor.iter().copied().collect();
        sorted.sort_by(f32::total_cmp);
        sorted.get(sorted.len() / 5).map_or(0.004, |f| (f * 2.5).max(0.004))
    }
}

enum Mode {
    Listening,
    Capturing { audio: Vec<f32>, started: f64, speech: bool, quiet: f64, threshold: f32 },
    Paused { since: f64 },
    Setup(Box<Setup>),
}

fn listen(app: AppHandle, cmds: mpsc::Receiver<Cmd>, ready: mpsc::SyncSender<Result<(), String>>, captured: Arc<Mutex<Option<Vec<f32>>>>, pending: Arc<Mutex<Option<Head>>>) {
    let (tx, rx) = mpsc::channel::<Vec<f32>>();
    let opened = match std::env::var("POB_REDUX_WAKE_WAV") {
        Ok(path) => play_file(&path, tx).map(|rate| (None, rate)),
        Err(_) => open_input(tx).map(|(s, rate)| (Some(s), rate)),
    };
    let (_stream, rate) = match opened {
        Ok(v) => v,
        Err(e) => {
            let _ = ready.send(Err(e));
            return;
        }
    };
    let mut resampler = match Fft::<f32>::new(rate as usize, RATE, 1024, 1, FixedSync::Input) {
        Ok(r) => r,
        Err(e) => {
            let _ = ready.send(Err(format!("The microphone rate {rate} Hz is not supported: {e}")));
            return;
        }
    };
    let _ = ready.send(Ok(()));
    let _ = app.emit("wake:state", "listening");

    let mut detector = Detector::new();
    let mut active = load_profile(&app).map(|p| p.head).unwrap_or_else(Head::generic);
    let mut generic = Head::generic();
    let (mut raw, mut ready16) = (Vec::<f32>::new(), Vec::<f32>::new());
    let mut ring: VecDeque<f32> = VecDeque::with_capacity(RATE * 2);
    let mut floor: VecDeque<f32> = VecDeque::with_capacity(32);
    let mut samples: u64 = 0;
    let mut mode = Mode::Listening;

    loop {
        while let Ok(cmd) = cmds.try_recv() {
            match cmd {
                Cmd::Stop => {
                    let _ = app.emit("wake:state", "stopped");
                    return;
                }
                Cmd::Resume => {
                    if matches!(mode, Mode::Paused { .. }) {
                        active.reset();
                        mode = Mode::Listening;
                        let _ = app.emit("wake:state", "listening");
                    }
                }
                Cmd::CancelCapture => {
                    if matches!(mode, Mode::Capturing { .. }) {
                        active.reset();
                        mode = Mode::Listening;
                        let _ = app.emit("wake:cancelled", "user");
                        let _ = app.emit("wake:state", "listening");
                    }
                }
                Cmd::SetupStart { tries, read_seconds } => {
                    let now = samples as f64 / RATE as f64;
                    mode = Mode::Setup(Box::new(Setup {
                        tries,
                        read_seconds,
                        test: false,
                        ask: Ask::Wait { until: now + 1.0, next: 1 },
                        user_pos: vec![],
                        best_per_try: vec![],
                        user_neg: vec![],
                        fitted: None,
                        heard: (0, 0),
                        floor: VecDeque::with_capacity(64),
                        silent: 0,
                        finish_read: false,
                    }));
                    let _ = app.emit("wake:state", "setup");
                }
                Cmd::SetupCancel => {
                    if matches!(mode, Mode::Setup(_)) {
                        mode = Mode::Listening;
                        let _ = app.emit("wake:state", "listening");
                    }
                }
                Cmd::SetupNext => {
                    if let Mode::Setup(s) = &mut mode {
                        s.finish_read = true;
                    }
                }
                Cmd::UseHead(head) => active = *head,
            }
        }
        match rx.recv_timeout(Duration::from_millis(100)) {
            Ok(chunk) => raw.extend(chunk),
            Err(mpsc::RecvTimeoutError::Timeout) => continue,
            Err(mpsc::RecvTimeoutError::Disconnected) => {
                let _ = app.emit("wake:state", "error");
                return;
            }
        }
        while raw.len() >= resampler.input_frames_next() {
            let n = resampler.input_frames_next();
            let Ok(input) = InterleavedSlice::new(&raw[..n], 1, n) else { break };
            match resampler.process(&input, None) {
                Ok(out) => ready16.extend(out.take_data()),
                Err(e) => log::warn!("wake: resample: {e}"),
            }
            raw.drain(..n);
        }
        while ready16.len() >= HOP {
            let chunk: Vec<f32> = ready16.drain(..HOP).collect();
            samples += HOP as u64;
            let now = samples as f64 / RATE as f64;
            ring.extend(chunk.iter().copied());
            while ring.len() > RATE * 2 {
                ring.pop_front();
            }
            let level = rms(&chunk);
            let e = detector.step(&chunk);
            let busy = crate::tts::speaking() || crate::voice::is_recording(&app);
            mode = match mode {
                Mode::Listening => {
                    floor.push_back(level);
                    if floor.len() > 30 {
                        floor.pop_front();
                    }
                    let fired = active.update(e.as_ref().map(|e| active.prob(e)), now);
                    if fired && !busy {
                        let mut sorted: Vec<f32> = floor.iter().copied().collect();
                        sorted.sort_by(f32::total_cmp);
                        let threshold = (sorted[sorted.len() / 5] * 2.5).max(0.004);
                        let pre = ((PRE_ROLL * RATE as f64) as usize).min(ring.len());
                        let audio: Vec<f32> = ring.iter().skip(ring.len() - pre).copied().collect();
                        let _ = app.emit("wake:triggered", ());
                        let _ = app.emit("wake:state", "capturing");
                        Mode::Capturing { audio, started: now, speech: false, quiet: 0.0, threshold }
                    } else {
                        Mode::Listening
                    }
                }
                Mode::Capturing { mut audio, started, mut speech, mut quiet, threshold } => {
                    audio.extend_from_slice(&chunk);
                    let _ = app.emit("wake:level", level);
                    let elapsed = now - started;
                    if elapsed > GRACE {
                        if level > threshold {
                            speech = true;
                            quiet = 0.0;
                        } else if speech {
                            quiet += HOP as f64 / RATE as f64;
                        }
                    }
                    if (speech && quiet >= END_SILENCE) || elapsed >= MAX_QUESTION {
                        let seconds = audio.len() as f64 / RATE as f64;
                        *captured.lock().unwrap() = Some(audio);
                        let _ = app.emit("wake:captured", seconds);
                        let _ = app.emit("wake:state", "paused");
                        Mode::Paused { since: now }
                    } else if !speech && elapsed >= WAIT_FOR_QUESTION {
                        let _ = app.emit("wake:cancelled", "silence");
                        let _ = app.emit("wake:state", "listening");
                        active.reset();
                        Mode::Listening
                    } else {
                        Mode::Capturing { audio, started, speech, quiet, threshold }
                    }
                }
                Mode::Paused { since } => {
                    active.update(None, now);
                    if now - since > AUTO_RESUME {
                        let _ = app.emit("wake:state", "listening");
                        Mode::Listening
                    } else {
                        Mode::Paused { since }
                    }
                }
                Mode::Setup(mut s) => {
                    let gp = e.as_ref().map(|e| generic.prob(e));
                    let g_fire = generic.update(gp, now);
                    let p_fire = match s.fitted.as_mut() {
                        Some(h) => {
                            let pp = e.as_ref().map(|e| h.prob(e));
                            h.update(pp, now)
                        }
                        None => false,
                    };
                    if !matches!(s.ask, Ask::Wait { .. }) {
                        let _ = app.emit("wake:level", level);
                    }
                    if step_setup(&app, &mut s, now, level, e, gp, g_fire, p_fire, &pending) {
                        Mode::Setup(s)
                    } else {
                        let _ = app.emit("wake:state", "listening");
                        active.reset();
                        Mode::Listening
                    }
                }
            };
        }
    }
}

/// Advances the voice setup by one hop; false once it has finished.
#[allow(clippy::too_many_arguments)]
fn step_setup(app: &AppHandle, s: &mut Setup, now: f64, level: f32, e: Option<Embedding>, gp: Option<f32>, g_fire: bool, p_fire: bool, pending: &Mutex<Option<Head>>) -> bool {
    let emit = |ev: SetupEvent| {
        let _ = app.emit("wake:setup", ev);
    };
    let hop = HOP as f64 / RATE as f64;
    let speech = s.speech();
    let total = if s.test { 3 } else { s.tries };
    let next = match std::mem::replace(&mut s.ask, Ask::Wait { until: 0.0, next: 0 }) {
        Ask::Wait { until, next } => {
            s.floor.push_back(level);
            if s.floor.len() > 60 {
                s.floor.pop_front();
            }
            if now >= until {
                emit(SetupEvent::Ask { attempt: next, total, test: s.test });
                Ask::Listen { attempt: next, started: now, end: None, windows: vec![], generic: false, personal: false, voiced: false, quiet: 0.0 }
            } else {
                Ask::Wait { until, next }
            }
        }
        Ask::Listen { attempt, started, mut end, mut windows, mut generic, mut personal, mut voiced, mut quiet } => {
            if let (Some(p), Some(e)) = (gp, e) {
                windows.push((p, e));
            }
            generic |= g_fire;
            personal |= p_fire;
            if level > speech {
                voiced = true;
                quiet = 0.0;
            } else if voiced {
                quiet += hop;
            }
            let fired = if s.test { personal } else { generic };
            if fired && end.is_none() {
                end = Some(now + 0.4);
            }
            let done = end.is_some_and(|t| now >= t) || (voiced && quiet >= TRY_END_SILENCE) || now - started > TRY_LIMIT;
            if !done {
                Ask::Listen { attempt, started, end, windows, generic, personal, voiced, quiet }
            } else {
                windows.sort_by(|a, b| b.0.total_cmp(&a.0));
                let best = windows.first().map_or(0.0, |w| w.0);
                let voice = voiced && !windows.is_empty();
                emit(SetupEvent::Result { attempt, total, test: s.test, voice, heard: generic, personal: s.test.then_some(personal), best });
                if !voice {
                    s.silent += 1;
                    if s.silent >= SILENT_TRIES {
                        emit(SetupEvent::Failed { error: "No speech was heard. Check that the right microphone is the default one and try again.".into() });
                        return false;
                    }
                    Ask::Wait { until: now + 1.0, next: attempt }
                } else {
                    s.silent = 0;
                    if s.test {
                        s.heard.0 += generic as usize;
                        s.heard.1 += personal as usize;
                    } else {
                        s.user_pos.extend(windows.iter().take(3).map(|w| w.1));
                        s.best_per_try.push(windows[0].1);
                    }
                    if attempt < total {
                        Ask::Wait { until: now + 1.3, next: attempt + 1 }
                    } else if s.test {
                        emit(SetupEvent::Done { generic: s.heard.0, personal: s.heard.1, tries: 3 });
                        *pending.lock().unwrap() = s.fitted.clone();
                        return false;
                    } else {
                        emit(SetupEvent::Read { needed: s.read_seconds });
                        s.finish_read = false;
                        Ask::Read { started: now, voiced: 0.0, quiet: 0.0, shown: now }
                    }
                }
            }
        }
        Ask::Read { started, mut voiced, mut quiet, mut shown } => {
            if let Some(e) = e {
                s.user_neg.push(e);
            }
            if level > speech {
                voiced += hop;
                quiet = 0.0;
            } else {
                quiet += hop;
            }
            if now - shown >= 0.5 {
                shown = now;
                emit(SetupEvent::Reading { voiced, needed: s.read_seconds });
            }
            let enough = voiced >= s.read_seconds;
            let done = (enough && quiet >= READ_END_SILENCE) || (s.finish_read && voiced >= s.read_seconds / 2.0) || now - started >= READ_LIMIT;
            if !done {
                Ask::Read { started, voiced, quiet, shown }
            } else if s.user_neg.len() < 20 {
                emit(SetupEvent::Failed { error: "Too little speech was heard while reading. Check the microphone and try again.".into() });
                return false;
            } else {
                let fit = fit_personal(&Head::generic(), &s.user_pos, &s.user_neg, &s.best_per_try);
                emit(SetupEvent::Fitted { min_positive: fit.min_positive, max_negative: fit.max_negative, threshold: fit.head.threshold, separated: fit.separated });
                s.fitted = Some(fit.head);
                s.test = true;
                Ask::Wait { until: now + 1.5, next: 1 }
            }
        }
    };
    s.ask = next;
    true
}
