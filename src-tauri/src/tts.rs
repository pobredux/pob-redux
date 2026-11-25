//! Reads assistant replies aloud with Phonon 31M, downloaded on request and run on this computer.

use std::collections::VecDeque;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use ptts::checkpoint::{Checkpoint, ResolveOptions};
use ptts::preprocess::Lang;
use ptts::synth::{DeviceKind, Quant, SpeechOptions, Synth};
use rubato::audioadapter_buffers::direct::InterleavedSlice;
use rubato::{Fft, FixedSync, Resampler};
use serde::Serialize;
use sha2::{Digest, Sha256};
use tauri::ipc::Channel;
use tauri::{AppHandle, Manager, State};

const MODEL_BASE: &str = "https://huggingface.co/gradium/phonon-31m-en/resolve/8f7a37c94a4c58b04c8287300c53f11d2e208eae";
const VOICE_BASE: &str = "https://huggingface.co/gradium/phonon-voices/resolve/0ee17205f254b5abc43b266c8a0f215565c352f7";

struct File {
    name: &'static str,
    bytes: u64,
    sha256: &'static str,
}

const MODEL_FILES: &[File] = &[
    File { name: "config.json", bytes: 4_634, sha256: "b5d0f88353638370d94035b7d73d7d6577c0554d78f8d7ecfd40490aae39ae85" },
    File { name: "generation_config.json", bytes: 220, sha256: "ffb086e8f531b10a43bfd17a56069f90b649171a55aae98b5e9a28c8411cffc0" },
    File { name: "processor_config.json", bytes: 747, sha256: "1ba08d75f0aba95fb494e8264e65e6b3e6b0b50981e1d49f183cb70eed58cfee" },
    File { name: "tokenizer.json", bytes: 277_967, sha256: "691a00068fa7b989f2f3e5ae5daa2d1a5bb319f2d5360deafcfb19a8bb6e7936" },
    File { name: "tokenizer_config.json", bytes: 575, sha256: "17d12cf8dee2891506648bb1d5657e26fb165263686d0e2ef311513488d6446f" },
    File { name: "default-voice.safetensors", bytes: 256_136, sha256: "dc537478eaadc1a5257fcda2e3501ab81e14c111a5d95a5838f350db2afd0f1a" },
    File { name: "model.q8.gguf", bytes: 64_512_352, sha256: "a7d9aad2cd6dc66473f24a08bc6d2939030ef4d751ef82349361fbef3272dbae" },
];

struct Voice {
    id: &'static str,
    name: &'static str,
    file: &'static str,
    sha256: &'static str,
}

const VOICE_BYTES: u64 = 256_136;
const VOICES: &[Voice] = &[
    Voice { id: "zoey", name: "Zoey", file: "NbpkqMVS3CJeq2j8", sha256: "97c7f8c253413574406ab80dd23f49c3ebb2517163b00a9bf86b9c4cf43e7e23" },
    Voice { id: "harper", name: "Harper", file: "4SZHfMpw-p46Ywgs", sha256: "6b3da6e2d796a20c51af892c482bd6509fb88267a221aff83c870843dee2a123" },
    Voice { id: "easton", name: "Easton", file: "LMDIYE0Oa377Qh6Z", sha256: "45c582673cb20edb04ddcca2b5a8865ddfb0a82599efadb269021e87d20fbdd4" },
    Voice { id: "russell", name: "Russell", file: "_6Aslh2DxfmnRLmP", sha256: "beae9e4030efce0a935fcb0f807dc0784d598bcaa87ad4aacb047436b1eaa411" },
    Voice { id: "maeve", name: "Maeve", file: "6PWnV0Nq4wu7RVBT", sha256: "56494ee67d533ad227ff0dfd0b1c13ff5aa91f1ad6e596e66c2986ba3005e844" },
    Voice { id: "hugh", name: "Hugh", file: "YyKcxPG7iIgS3jsK", sha256: "3169d74b47a54d1eb15e4c243f56393abe9fe90c7e1c308ecf96ee67618b152a" },
    Voice { id: "toby", name: "Toby", file: "dME3IWyZBvmh1n1q", sha256: "09ab315c7f4b72b4d71e66cb756184cedfb1f14d222d096e115bf95bfe7b29dc" },
    Voice { id: "aldric", name: "Master Aldric", file: "gSgG5Tor0GPEnqzC", sha256: "a83430cc022b85c83753f859f9e44f406651646dab994fced8ef7a9a88005971" },
    Voice { id: "mortlake", name: "Old Mortlake", file: "lX3LavMNdlIbgEHH", sha256: "d3f9429205e5906dcd3c78edaf7c9d5148c7e389e34c82c86f72f154adb5d923" },
];

static SPEAKING: AtomicBool = AtomicBool::new(false);
static CURRENT: AtomicU64 = AtomicU64::new(0);

/// True while a reply is playing, so the wake word does not hear the app's own voice.
pub fn speaking() -> bool {
    SPEAKING.load(Ordering::Relaxed)
}

#[derive(Default)]
pub struct TtsState {
    synth: Mutex<Option<Arc<Synth>>>,
}

#[derive(Serialize)]
pub struct VoiceStatus {
    id: &'static str,
    name: &'static str,
    installed: bool,
}

#[derive(Serialize)]
pub struct TtsStatus {
    installed: bool,
    bytes: u64,
    voices: Vec<VoiceStatus>,
}

#[derive(Serialize, Clone)]
pub struct Progress {
    done: u64,
    total: u64,
}

fn model_dir(app: &AppHandle) -> Result<PathBuf, String> {
    Ok(app.path().app_local_data_dir().map_err(|e| e.to_string())?.join("models").join("phonon-31m-en"))
}

fn has(path: &Path, bytes: u64) -> bool {
    std::fs::metadata(path).is_ok_and(|m| m.len() == bytes)
}

fn installed(dir: &Path) -> bool {
    MODEL_FILES.iter().all(|f| has(&dir.join(f.name), f.bytes))
}

fn voice_path(dir: &Path, v: &Voice) -> PathBuf {
    dir.join("voices").join(format!("{}.safetensors", v.id))
}

#[tauri::command]
pub fn tts_status(app: AppHandle) -> TtsStatus {
    let dir = model_dir(&app).unwrap_or_default();
    TtsStatus {
        installed: installed(&dir),
        bytes: MODEL_FILES.iter().map(|f| f.bytes).sum(),
        voices: VOICES.iter().map(|v| VoiceStatus { id: v.id, name: v.name, installed: has(&voice_path(&dir, v), VOICE_BYTES) }).collect(),
    }
}

async fn download(client: &reqwest::Client, url: &str, path: &Path, bytes: u64, sha256: &str, mut progress: impl FnMut(u64)) -> Result<(), String> {
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    }
    let part = path.with_extension("part");
    let mut res = client.get(url).send().await.map_err(|e| format!("could not reach Hugging Face: {e}"))?;
    if !res.status().is_success() {
        return Err(format!("Hugging Face returned {}", res.status()));
    }
    let mut file = std::fs::File::create(&part).map_err(|e| format!("{}: {e}", part.display()))?;
    let mut hash = Sha256::new();
    let mut done = 0u64;
    while let Some(chunk) = res.chunk().await.map_err(|e| format!("the download stopped: {e}"))? {
        hash.update(&chunk);
        file.write_all(&chunk).map_err(|e| format!("{}: {e}", part.display()))?;
        done += chunk.len() as u64;
        progress(done);
    }
    drop(file);
    let digest: String = hash.finalize().iter().map(|b| format!("{b:02x}")).collect();
    if done != bytes || digest != sha256 {
        let _ = std::fs::remove_file(&part);
        return Err("The download does not match the published file, so it was deleted. Try again.".into());
    }
    std::fs::rename(&part, path).map_err(|e| format!("{}: {e}", path.display()))
}

fn client() -> Result<reqwest::Client, String> {
    reqwest::Client::builder().user_agent(concat!("pob-redux/", env!("CARGO_PKG_VERSION"))).build().map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn tts_install(app: AppHandle, state: State<'_, TtsState>, on_progress: Channel<Progress>) -> Result<(), String> {
    let dir = model_dir(&app)?;
    let client = client()?;
    let total: u64 = MODEL_FILES.iter().map(|f| f.bytes).sum();
    let mut before = 0u64;
    let mut reported = u64::MAX;
    for f in MODEL_FILES {
        let path = dir.join(f.name);
        if !has(&path, f.bytes) {
            download(&client, &format!("{MODEL_BASE}/{}", f.name), &path, f.bytes, f.sha256, |done| {
                let pct = (before + done) * 100 / total;
                if pct != reported {
                    reported = pct;
                    let _ = on_progress.send(Progress { done: before + done, total });
                }
            })
            .await?;
        }
        before += f.bytes;
    }
    *state.synth.lock().unwrap() = None;
    Ok(())
}

#[tauri::command]
pub async fn tts_voice_install(app: AppHandle, state: State<'_, TtsState>, voice: String) -> Result<(), String> {
    let v = VOICES.iter().find(|v| v.id == voice).ok_or_else(|| format!("Unknown voice: {voice}"))?;
    let path = voice_path(&model_dir(&app)?, v);
    if !has(&path, VOICE_BYTES) {
        download(&client()?, &format!("{VOICE_BASE}/voices/{}.safetensors", v.file), &path, VOICE_BYTES, v.sha256, |_| {}).await?;
        *state.synth.lock().unwrap() = None;
    }
    Ok(())
}

#[tauri::command]
pub fn tts_remove(app: AppHandle, state: State<'_, TtsState>) -> Result<(), String> {
    CURRENT.fetch_add(1, Ordering::SeqCst);
    *state.synth.lock().unwrap() = None;
    let dir = model_dir(&app)?;
    match std::fs::remove_dir_all(&dir) {
        Ok(()) => Ok(()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(e) => Err(format!("{}: {e}", dir.display())),
    }
}

fn synth(app: &AppHandle) -> Result<Arc<Synth>, String> {
    let state = app.state::<TtsState>();
    let mut slot = state.synth.lock().unwrap();
    if let Some(s) = slot.as_ref() {
        return Ok(s.clone());
    }
    let dir = model_dir(app)?;
    if !installed(&dir) {
        return Err("The speech voice is not downloaded. Download it in Settings > Experimental.".into());
    }
    let fail = |e: ptts::Error| format!("The speech voice could not be loaded: {e}");
    let checkpoint = Checkpoint::resolve(&dir, ResolveOptions { quant: Quant::Q80, weights: None }).map_err(fail)?;
    let mut builder = checkpoint.builder(Lang::En).device(DeviceKind::Cpu);
    for v in VOICES {
        let path = voice_path(&dir, v);
        if has(&path, VOICE_BYTES) {
            builder = builder.add_voice(v.id, path);
        }
    }
    let mut synth = builder.build().map_err(fail)?;
    checkpoint.register_voices(&mut synth);
    let synth = Arc::new(synth);
    *slot = Some(synth.clone());
    Ok(synth)
}

type Queue = Arc<Mutex<VecDeque<f32>>>;

fn open_output(queue: Queue) -> Result<(cpal::Stream, u32), String> {
    let device = cpal::default_host().default_output_device().ok_or("No speakers or headphones were found.")?;
    let supported = device.default_output_config().map_err(|e| format!("The speakers could not be opened: {e}"))?;
    let rate = supported.sample_rate();
    let channels = supported.channels().max(1) as usize;
    let format = supported.sample_format();
    let config: cpal::StreamConfig = supported.into();
    fn build<T>(device: &cpal::Device, config: cpal::StreamConfig, channels: usize, queue: Queue) -> Result<cpal::Stream, cpal::Error>
    where
        T: cpal::SizedSample + cpal::FromSample<f32>,
    {
        device.build_output_stream(
            config,
            move |data: &mut [T], _: &cpal::OutputCallbackInfo| {
                let mut q = queue.lock().unwrap();
                for frame in data.chunks_mut(channels) {
                    let s = q.pop_front().unwrap_or(0.0);
                    frame.iter_mut().for_each(|x| *x = T::from_sample(s));
                }
            },
            |e| log::warn!("tts: speakers: {e}"),
            None,
        )
    }
    let stream = match format {
        cpal::SampleFormat::F32 => build::<f32>(&device, config, channels, queue),
        cpal::SampleFormat::I16 => build::<i16>(&device, config, channels, queue),
        cpal::SampleFormat::I32 => build::<i32>(&device, config, channels, queue),
        cpal::SampleFormat::U16 => build::<u16>(&device, config, channels, queue),
        other => return Err(format!("The speakers use a sample format this app cannot write ({other:?}).")),
    }
    .map_err(|e| format!("The speakers could not be opened: {e}"))?;
    stream.play().map_err(|e| format!("The speakers could not start: {e}"))?;
    Ok((stream, rate))
}

fn speak(app: &AppHandle, text: &str, voice: &str, id: u64) -> Result<(), String> {
    let synth = synth(app)?;
    let queue: Queue = Arc::new(Mutex::new(VecDeque::new()));
    let (_stream, out_rate) = open_output(queue.clone())?;
    let opts = SpeechOptions::default().voice(if voice.is_empty() { "default" } else { voice });
    let stream = synth.stream_with(text, &opts).map_err(|e| format!("Speech failed: {e}"))?;
    let in_rate = stream.sample_rate() as usize;
    let mut resampler = Fft::<f32>::new(in_rate, out_rate as usize, 1024, 1, FixedSync::Input).map_err(|e| e.to_string())?;
    let live = || CURRENT.load(Ordering::SeqCst) == id;
    let mut pending: Vec<f32> = Vec::new();
    let mut push = |pending: &mut Vec<f32>, flush: bool| {
        if flush && !pending.is_empty() {
            let n = resampler.input_frames_next();
            pending.resize(pending.len().div_ceil(n) * n, 0.0);
        }
        while pending.len() >= resampler.input_frames_next() {
            let n = resampler.input_frames_next();
            if let Ok(input) = InterleavedSlice::new(&pending[..n], 1, n) {
                if let Ok(out) = resampler.process(&input, None) {
                    queue.lock().unwrap().extend(out.take_data());
                }
            }
            pending.drain(..n);
        }
    };
    for chunk in stream {
        if !live() {
            break;
        }
        pending.extend(chunk.map_err(|e| format!("Speech failed: {e}"))?);
        push(&mut pending, false);
    }
    push(&mut pending, true);
    while live() && !queue.lock().unwrap().is_empty() {
        std::thread::sleep(Duration::from_millis(30));
    }
    queue.lock().unwrap().clear();
    std::thread::sleep(Duration::from_millis(150));
    Ok(())
}

#[tauri::command]
pub async fn tts_speak(app: AppHandle, text: String, voice: String) -> Result<(), String> {
    let id = CURRENT.fetch_add(1, Ordering::SeqCst) + 1;
    SPEAKING.store(true, Ordering::SeqCst);
    let result = tauri::async_runtime::spawn_blocking(move || speak(&app, &text, &voice, id)).await.map_err(|e| e.to_string());
    if CURRENT.load(Ordering::SeqCst) == id {
        tokio::time::sleep(Duration::from_millis(300)).await;
        SPEAKING.store(false, Ordering::SeqCst);
    }
    result?
}

#[tauri::command]
pub fn tts_stop() {
    CURRENT.fetch_add(1, Ordering::SeqCst);
    SPEAKING.store(false, Ordering::SeqCst);
}
