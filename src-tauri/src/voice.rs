//! Voice input for the assistant: records the microphone and transcribes it on this computer with Whisper.

use std::io::Write;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{mpsc, Arc, Mutex};
use std::time::Duration;

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use serde::Serialize;
use sha2::{Digest, Sha256};
use tauri::ipc::Channel;
use tauri::{AppHandle, Manager, State};
use whisper_rs::{FullParams, SamplingStrategy, WhisperContext, WhisperContextParameters};

struct Model {
    file: &'static str,
    url: &'static str,
    bytes: u64,
    sha256: &'static str,
}

const MODEL: Model = Model {
    file: "ggml-base.en.bin",
    url: "https://huggingface.co/ggerganov/whisper.cpp/resolve/5359861c739e955e79d9a303bcbc70fb988958b1/ggml-base.en.bin",
    bytes: 147_964_211,
    sha256: "a03779c86df3323075f5e796cb2ce5029f00ec8869eee3fdfb897afe36c6d002",
};

const RATE: usize = 16_000;
const MAX_SECONDS: usize = 120;
const WINDOW: usize = RATE / 50;
const SILENCE: f32 = 0.004;

struct Captured {
    samples: Vec<f32>,
    rate: u32,
}

struct Recording {
    stop: mpsc::Sender<()>,
    done: std::thread::JoinHandle<Result<Captured, String>>,
}

impl Recording {
    fn finish(self) -> Result<Captured, String> {
        let _ = self.stop.send(());
        self.done.join().map_err(|_| "The recorder stopped unexpectedly.".to_string())?
    }
}

#[derive(Default)]
pub struct VoiceState {
    recording: Mutex<Option<Recording>>,
    model: Mutex<Option<Arc<WhisperContext>>>,
}

impl VoiceState {
    pub fn new() -> Self {
        whisper_rs::install_logging_hooks();
        Self::default()
    }
}

#[derive(Serialize)]
pub struct VoiceStatus {
    installed: bool,
    bytes: u64,
}

#[derive(Serialize, Clone)]
pub struct Progress {
    done: u64,
    total: u64,
}

fn model_path(app: &AppHandle) -> Result<PathBuf, String> {
    Ok(app.path().app_local_data_dir().map_err(|e| e.to_string())?.join("models").join(MODEL.file))
}

fn installed(app: &AppHandle) -> bool {
    model_path(app).ok().and_then(|p| std::fs::metadata(p).ok()).is_some_and(|m| m.len() == MODEL.bytes)
}

#[tauri::command]
pub fn voice_status(app: AppHandle) -> VoiceStatus {
    VoiceStatus { installed: installed(&app), bytes: MODEL.bytes }
}

#[tauri::command]
pub async fn voice_install(app: AppHandle, on_progress: Channel<Progress>) -> Result<(), String> {
    let path = model_path(&app)?;
    let dir = path.parent().ok_or("no model folder")?;
    std::fs::create_dir_all(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    let part = path.with_extension("bin.part");

    let client = reqwest::Client::builder()
        .user_agent(concat!("pob-redux/", env!("CARGO_PKG_VERSION")))
        .build()
        .map_err(|e| e.to_string())?;
    let mut res = client.get(MODEL.url).send().await.map_err(|e| format!("could not reach Hugging Face: {e}"))?;
    if !res.status().is_success() {
        return Err(format!("Hugging Face returned {}", res.status()));
    }
    let mut file = std::fs::File::create(&part).map_err(|e| format!("{}: {e}", part.display()))?;
    let mut hash = Sha256::new();
    let mut done: u64 = 0;
    let mut reported = u64::MAX;
    while let Some(chunk) = res.chunk().await.map_err(|e| format!("the download stopped: {e}"))? {
        hash.update(&chunk);
        file.write_all(&chunk).map_err(|e| format!("{}: {e}", part.display()))?;
        done += chunk.len() as u64;
        let pct = done * 100 / MODEL.bytes;
        if pct != reported {
            reported = pct;
            let _ = on_progress.send(Progress { done, total: MODEL.bytes });
        }
    }
    drop(file);
    let digest: String = hash.finalize().iter().map(|b| format!("{b:02x}")).collect();
    if done != MODEL.bytes || digest != MODEL.sha256 {
        let _ = std::fs::remove_file(&part);
        return Err("The download does not match the published model, so it was deleted. Try again.".into());
    }
    std::fs::rename(&part, &path).map_err(|e| format!("{}: {e}", path.display()))
}

#[tauri::command]
pub fn voice_remove(app: AppHandle, state: State<'_, VoiceState>) -> Result<(), String> {
    *state.model.lock().unwrap() = None;
    let path = model_path(&app)?;
    match std::fs::remove_file(&path) {
        Ok(()) => Ok(()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(e) => Err(format!("{}: {e}", path.display())),
    }
}

#[tauri::command]
pub fn voice_start(app: AppHandle, state: State<'_, VoiceState>, on_level: Channel<f32>) -> Result<(), String> {
    if !installed(&app) {
        return Err("The speech model is not downloaded. Download it in Settings > Experimental.".into());
    }
    let mut slot = state.recording.lock().unwrap();
    if let Some(old) = slot.take() {
        let _ = old.finish();
    }
    let (stop, stopped) = mpsc::channel();
    let (ready_tx, ready) = mpsc::sync_channel(1);
    let done = std::thread::Builder::new()
        .name("voice-record".into())
        .spawn(move || record(stopped, ready_tx, on_level))
        .map_err(|e| e.to_string())?;
    if let Ok(Err(e)) | Err(e) = ready.recv().map_err(|_| "The microphone could not be opened.".to_string()) {
        let _ = done.join();
        return Err(e);
    }
    *slot = Some(Recording { stop, done });
    let handle = app.clone();
    tauri::async_runtime::spawn_blocking(move || {
        if let Err(e) = load_model(&handle) {
            log::warn!("voice: {e}");
        }
    });
    Ok(())
}

#[tauri::command]
pub async fn voice_stop(app: AppHandle, prompt: String) -> Result<String, String> {
    let rec = app.state::<VoiceState>().recording.lock().unwrap().take().ok_or("Nothing is being recorded.")?;
    tauri::async_runtime::spawn_blocking(move || -> Result<String, String> {
        let captured = rec.finish()?;
        let audio = resample(&captured.samples, captured.rate);
        let Some(span) = speech_span(&audio) else {
            return Ok(String::new());
        };
        let ctx = load_model(&app)?;
        transcribe(&ctx, &audio[span], &prompt)
    })
    .await
    .map_err(|e| e.to_string())?
}

#[tauri::command]
pub fn voice_cancel(state: State<'_, VoiceState>) {
    if let Some(rec) = state.recording.lock().unwrap().take() {
        let _ = rec.finish();
    }
}

fn record(stop: mpsc::Receiver<()>, ready: mpsc::SyncSender<Result<(), String>>, on_level: Channel<f32>) -> Result<Captured, String> {
    let (stream, buf, level, rate) = match open_stream() {
        Ok(v) => v,
        Err(e) => {
            let _ = ready.send(Err(e.clone()));
            return Err(e);
        }
    };
    let _ = ready.send(Ok(()));
    while let Err(mpsc::RecvTimeoutError::Timeout) = stop.recv_timeout(Duration::from_millis(50)) {
        let _ = on_level.send(f32::from_bits(level.load(Ordering::Relaxed)));
    }
    drop(stream);
    let samples = std::mem::take(&mut *buf.lock().unwrap());
    Ok(Captured { samples, rate })
}

type Buffer = Arc<Mutex<Vec<f32>>>;

fn open_stream() -> Result<(cpal::Stream, Buffer, Arc<AtomicU32>, u32), String> {
    let device = cpal::default_host().default_input_device().ok_or("No microphone was found.")?;
    let supported = device.default_input_config().map_err(|e| format!("The microphone could not be opened: {e}"))?;
    let rate = supported.sample_rate();
    let channels = supported.channels().max(1) as usize;
    let format = supported.sample_format();
    let config: cpal::StreamConfig = supported.into();
    let buf: Buffer = Arc::new(Mutex::new(Vec::with_capacity(rate as usize * 10)));
    let level = Arc::new(AtomicU32::new(0));
    let cap = rate as usize * MAX_SECONDS;
    let stream = match format {
        cpal::SampleFormat::F32 => build::<f32>(&device, config, channels, cap, &buf, &level),
        cpal::SampleFormat::I16 => build::<i16>(&device, config, channels, cap, &buf, &level),
        cpal::SampleFormat::I32 => build::<i32>(&device, config, channels, cap, &buf, &level),
        cpal::SampleFormat::U16 => build::<u16>(&device, config, channels, cap, &buf, &level),
        cpal::SampleFormat::U8 => build::<u8>(&device, config, channels, cap, &buf, &level),
        other => return Err(format!("The microphone uses a sample format this app cannot read ({other:?}).")),
    }
    .map_err(|e| format!("The microphone could not be opened: {e}"))?;
    stream.play().map_err(|e| format!("The microphone could not start: {e}"))?;
    Ok((stream, buf, level, rate))
}

fn build<T>(
    device: &cpal::Device,
    config: cpal::StreamConfig,
    channels: usize,
    cap: usize,
    buf: &Buffer,
    level: &Arc<AtomicU32>,
) -> Result<cpal::Stream, cpal::Error>
where
    T: cpal::SizedSample,
    f32: cpal::FromSample<T>,
{
    let (buf, level) = (buf.clone(), level.clone());
    device.build_input_stream(
        config,
        move |data: &[T], _: &cpal::InputCallbackInfo| {
            let mut out = buf.lock().unwrap();
            let mut power = 0.0f32;
            let mut frames = 0usize;
            for frame in data.chunks(channels) {
                let s = frame.iter().map(|x| x.to_sample::<f32>()).sum::<f32>() / frame.len() as f32;
                power += s * s;
                frames += 1;
                if out.len() < cap {
                    out.push(s);
                }
            }
            level.store((power / frames.max(1) as f32).sqrt().to_bits(), Ordering::Relaxed);
        },
        |e| log::warn!("voice: microphone: {e}"),
        None,
    )
}

/// Whisper wants 16 kHz; averaging each output sample's span of input is enough of a low-pass for speech.
fn resample(input: &[f32], from: u32) -> Vec<f32> {
    if from as usize == RATE || input.is_empty() {
        return input.to_vec();
    }
    let ratio = from as f64 / RATE as f64;
    let n = (input.len() as f64 / ratio) as usize;
    (0..n)
        .map(|i| {
            let start = ((i as f64 * ratio) as usize).min(input.len() - 1);
            let end = (((i + 1) as f64 * ratio) as usize).clamp(start + 1, input.len());
            input[start..end].iter().sum::<f32>() / (end - start) as f32
        })
        .collect()
}

/// Whisper turns steady noise into words, so it only gets the part of the clip well above the clip's own noise floor.
fn speech_span(audio: &[f32]) -> Option<std::ops::Range<usize>> {
    let rms: Vec<f32> = audio.chunks(WINDOW).map(|w| (w.iter().map(|s| s * s).sum::<f32>() / w.len() as f32).sqrt()).collect();
    let mut sorted = rms.clone();
    sorted.sort_by(f32::total_cmp);
    let gate = (sorted.get(sorted.len() / 10).copied().unwrap_or(0.0) * 3.0).max(SILENCE);
    let loud: Vec<usize> = rms.iter().enumerate().filter(|(_, r)| **r > gate).map(|(i, _)| i).collect();
    if loud.len() < 8 {
        return None;
    }
    let pad = 15;
    let start = loud[0].saturating_sub(pad) * WINDOW;
    let end = ((loud[loud.len() - 1] + 1 + pad) * WINDOW).min(audio.len());
    Some(start..end)
}

fn load_model(app: &AppHandle) -> Result<Arc<WhisperContext>, String> {
    let state = app.state::<VoiceState>();
    let mut slot = state.model.lock().unwrap();
    if let Some(ctx) = slot.as_ref() {
        return Ok(ctx.clone());
    }
    let path = model_path(app)?;
    let bytes = std::fs::read(&path).map_err(|e| format!("{}: {e}", path.display()))?;
    let ctx = Arc::new(open_model(&bytes)?);
    *slot = Some(ctx.clone());
    Ok(ctx)
}

// whisper.cpp opens files by narrow path, which breaks on non-ASCII Windows profiles, so it gets the bytes.
fn open_model(bytes: &[u8]) -> Result<WhisperContext, String> {
    WhisperContext::new_from_buffer_with_params(bytes, WhisperContextParameters::default())
        .map_err(|e| format!("The speech model could not be loaded: {e}"))
}

fn transcribe(ctx: &WhisperContext, audio: &[f32], prompt: &str) -> Result<String, String> {
    let mut state = ctx.create_state().map_err(|e| format!("The speech model could not start: {e}"))?;
    let mut params = FullParams::new(SamplingStrategy::Greedy { best_of: 1 });
    params.set_language(Some("en"));
    params.set_n_threads(std::thread::available_parallelism().map_or(4, |n| n.get()).clamp(1, 8) as i32);
    params.set_no_context(true);
    params.set_no_timestamps(true);
    params.set_suppress_nst(true);
    params.set_print_special(false);
    params.set_print_progress(false);
    params.set_print_realtime(false);
    params.set_print_timestamps(false);
    let prompt = prompt.replace('\0', "");
    if !prompt.trim().is_empty() {
        params.set_initial_prompt(&prompt);
    }
    // whisper.cpp returns nothing for less than a second of audio.
    let min = RATE * 5 / 4;
    let mut padded;
    let audio = if audio.len() < min {
        padded = audio.to_vec();
        padded.resize(min, 0.0);
        &padded[..]
    } else {
        audio
    };
    state.full(params, audio).map_err(|e| format!("Transcription failed: {e}"))?;
    let text: Vec<String> = state
        .as_iter()
        .filter_map(|s| s.to_str_lossy().ok().map(|t| t.trim().to_string()))
        .filter(|t| t.chars().any(char::is_alphanumeric) && !is_marker(t) && !is_hallucination(t))
        .collect();
    Ok(text.join(" "))
}

fn is_marker(t: &str) -> bool {
    (t.starts_with('[') && t.ends_with(']')) || (t.starts_with('(') && t.ends_with(')'))
}

/// Phrases Whisper invents from background sound, learned from subtitled video.
fn is_hallucination(t: &str) -> bool {
    let t = t.to_lowercase();
    let bare = t.trim_matches(|c: char| !c.is_alphanumeric());
    ["subtitles by", "subs by", "thanks for watching", "thank you for watching", "please subscribe", "amara.org", "www.", ".co.uk", ".com"]
        .iter()
        .any(|p| t.contains(p))
        || ["you", "end", "the end", "bye"].contains(&bare)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resample_keeps_duration() {
        let one_second = vec![0.5f32; 48_000];
        let out = resample(&one_second, 48_000);
        assert_eq!(out.len(), RATE);
        assert!(out.iter().all(|s| (s - 0.5).abs() < 1e-6));
        assert_eq!(resample(&vec![0.0; 44_100], 44_100).len(), RATE);
        assert_eq!(resample(&vec![0.0; 8_000], 8_000).len(), RATE);
    }

    #[test]
    fn speech_span_skips_silence_and_steady_noise() {
        assert_eq!(speech_span(&vec![0.0; RATE * 2]), None);
        let hum: Vec<f32> = (0..RATE * 2).map(|i| (i as f32 * 0.3).sin() * 0.05).collect();
        assert_eq!(speech_span(&hum), None);
        let mut clip = vec![0.001f32; RATE * 3];
        for (i, s) in clip[RATE..RATE * 2].iter_mut().enumerate() {
            *s = (i as f32 * 0.05).sin() * 0.2;
        }
        let span = speech_span(&clip).unwrap();
        assert!(span.start <= RATE && span.start >= RATE / 2);
        assert!(span.end >= RATE * 2 && span.end <= RATE * 5 / 2);
    }

    #[test]
    fn markers_are_dropped() {
        assert!(is_marker("[BLANK_AUDIO]"));
        assert!(is_marker("(wind blowing)"));
        assert!(!is_marker("Equip Adonia's Ego."));
        assert!(is_hallucination("Subs by www.zeoranger.co.uk"));
        assert!(is_hallucination(" The End."));
        assert!(is_hallucination("End"));
        assert!(!is_hallucination("Thank you, now equip Adonia's Ego."));
    }

    /// `VOICE_MODEL=<ggml .bin> VOICE_WAV=<16-bit PCM .wav> cargo test -p pob-redux --lib voice -- --ignored --nocapture`
    #[test]
    #[ignore]
    fn transcribes_a_wav() {
        let model = std::fs::read(std::env::var("VOICE_MODEL").unwrap()).unwrap();
        let wav = std::fs::read(std::env::var("VOICE_WAV").unwrap()).unwrap();
        let (mut channels, mut rate, mut samples) = (1usize, 16_000u32, Vec::new());
        let mut at = 12;
        while at + 8 <= wav.len() {
            let id = &wav[at..at + 4];
            let len = u32::from_le_bytes(wav[at + 4..at + 8].try_into().unwrap()) as usize;
            let body = &wav[at + 8..(at + 8 + len).min(wav.len())];
            if id == b"fmt " {
                channels = u16::from_le_bytes([body[2], body[3]]) as usize;
                rate = u32::from_le_bytes(body[4..8].try_into().unwrap());
            } else if id == b"data" {
                samples = body.chunks_exact(2).map(|b| i16::from_le_bytes([b[0], b[1]]) as f32 / 32768.0).collect();
            }
            at += 8 + len + (len & 1);
        }
        let mono: Vec<f32> = samples.chunks(channels).map(|f| f.iter().sum::<f32>() / f.len() as f32).collect();
        let audio = resample(&mono, rate);
        let Some(span) = speech_span(&audio) else {
            println!("no speech");
            return;
        };
        let audio = &audio[span];
        let ctx = open_model(&model).unwrap();
        let prompt = std::env::var("VOICE_PROMPT").unwrap_or_default();
        let started = std::time::Instant::now();
        let text = transcribe(&ctx, audio, &prompt).unwrap();
        println!("{:.2}s audio in {:?}: {text}", audio.len() as f32 / RATE as f32, started.elapsed());
    }
}
