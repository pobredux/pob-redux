//! The "Hey Redux" detector: Heed's preprocessing and network in Rust; `model/` holds weights with batch norm folded in.

use std::f64::consts::PI;
use std::sync::Arc;

use rustfft::num_complex::Complex;
use rustfft::{Fft, FftPlanner};
use serde::{Deserialize, Serialize};

pub const RATE: usize = 16_000;
pub const WINDOW: usize = RATE;
pub const HOP: usize = 1_600;
const N_FFT: usize = 512;
const WIN: usize = 400;
const STFT_HOP: usize = 160;
const N_MELS: usize = 40;
const FRAMES: usize = WINDOW / STFT_HOP + 1;
pub const DIM: usize = 64;
const STEM_KERNEL: usize = 5;
const BLOCK_KERNEL: usize = 7;
const STRIDES: [usize; 3] = [1, 2, 1];

const NET: &[u8] = include_bytes!("model/net.f32");
const HEAD: &str = include_str!("model/head.json");
const BANK: &[u8] = include_bytes!("model/bank.f32");

pub type Embedding = [f32; DIM];

fn floats(bytes: &[u8]) -> Vec<f32> {
    bytes.as_chunks::<4>().0.iter().map(|b| f32::from_le_bytes(*b)).collect()
}

#[derive(Deserialize)]
struct HeadFile {
    weight: Vec<f32>,
    bias: f32,
    threshold: f32,
    filter: Vec<Vec<[f64; 7]>>,
}

fn head_file() -> HeadFile {
    serde_json::from_str(HEAD).expect("wake head")
}

struct Biquad {
    b: [f64; 3],
    a: [f64; 2],
    zi: [f64; 2],
    z: Option<[f64; 2]>,
}

/// scipy `sosfilt` with each group's state primed from its own first input sample, as Heed streams it.
struct Filter {
    groups: Vec<Vec<Biquad>>,
}

impl Filter {
    fn new() -> Self {
        let groups = head_file()
            .filter
            .into_iter()
            .map(|g| g.into_iter().map(|s| Biquad { b: [s[0], s[1], s[2]], a: [s[3], s[4]], zi: [s[5], s[6]], z: None }).collect())
            .collect();
        Filter { groups }
    }

    fn run(&mut self, input: &[f32]) -> Vec<f32> {
        let mut x: Vec<f64> = input.iter().map(|&s| s as f64).collect();
        if x.is_empty() {
            return vec![];
        }
        for group in &mut self.groups {
            let first = x[0];
            for stage in group.iter_mut() {
                let z = stage.z.get_or_insert([stage.zi[0] * first, stage.zi[1] * first]);
                for s in x.iter_mut() {
                    let y = stage.b[0] * *s + z[0];
                    z[0] = stage.b[1] * *s - stage.a[0] * y + z[1];
                    z[1] = stage.b[2] * *s - stage.a[1] * y;
                    *s = y;
                }
            }
        }
        x.into_iter().map(|s| s as f32).collect()
    }
}

fn hz_to_mel(hz: f64) -> f64 {
    2595.0 * (1.0 + hz / 700.0).log10()
}

fn mel_to_hz(m: f64) -> f64 {
    700.0 * (10f64.powf(m / 2595.0) - 1.0)
}

/// torch.linspace in float32, which counts the second half down from the end.
fn linspace(a: f32, b: f32, n: usize) -> Vec<f32> {
    let step = (b - a) / (n - 1) as f32;
    (0..n).map(|i| if i < n / 2 { a + step * i as f32 } else { b - step * (n - 1 - i) as f32 }).collect()
}

struct Features {
    fft: Arc<dyn Fft<f32>>,
    gate_fft: Arc<dyn Fft<f32>>,
    window: Vec<f32>,
    gate_window: Vec<f32>,
    fb: Vec<[f32; N_FFT / 2 + 1]>,
}

impl Features {
    fn new() -> Self {
        let mut planner = FftPlanner::new();
        let pad = (N_FFT - WIN) / 2;
        let mut window = vec![0f32; N_FFT];
        for i in 0..WIN {
            window[pad + i] = (0.5 - 0.5 * (2.0 * PI * i as f64 / WIN as f64).cos()) as f32;
        }
        let gate_window = (0..WINDOW).map(|i| (0.5 - 0.5 * (2.0 * PI * i as f64 / WINDOW as f64).cos()) as f32).collect();
        let mel_pts = linspace(hz_to_mel(0.0) as f32, hz_to_mel(8000.0) as f32, N_MELS + 2);
        let hz: Vec<f32> = mel_pts.iter().map(|&m| mel_to_hz(m as f64) as f32).collect();
        let freqs = linspace(0.0, 8000.0, N_FFT / 2 + 1);
        let fb = (0..N_MELS)
            .map(|m| {
                let (l, c, r) = (hz[m], hz[m + 1], hz[m + 2]);
                let mut row = [0f32; N_FFT / 2 + 1];
                for (k, &f) in freqs.iter().enumerate() {
                    row[k] = ((f - l) / (c - l + 1e-9)).min((r - f) / (r - c + 1e-9)).max(0.0);
                }
                row
            })
            .collect();
        Features { fft: planner.plan_fft_forward(N_FFT), gate_fft: planner.plan_fft_forward(WINDOW), window, gate_window, fb }
    }

    fn gate(&self, x: &[f32]) -> bool {
        let rms = (x.iter().map(|s| s * s).sum::<f32>() / x.len() as f32).sqrt();
        if rms < 10f32.powf(-55.0 / 20.0) {
            return false;
        }
        let mut buf: Vec<Complex<f32>> = x.iter().zip(&self.gate_window).map(|(s, w)| Complex::new(s * w, 0.0)).collect();
        self.gate_fft.process(&mut buf);
        let (mut total, mut voice) = (0f32, 0f32);
        for (k, c) in buf.iter().take(WINDOW / 2 + 1).enumerate() {
            let p = c.norm_sqr();
            let f = k as f32 * RATE as f32 / WINDOW as f32;
            total += p;
            if (100.0..=7000.0).contains(&f) {
                voice += p;
            }
        }
        voice / total.max(1e-12) >= 0.15
    }

    /// Log-mel with reflect padding and per-bin mean removal, laid out `[mel][frame]`.
    fn log_mel(&self, x: &[f32]) -> Vec<f32> {
        let half = N_FFT / 2;
        let n = x.len() as isize;
        let at = |i: isize| -> f32 {
            let j = if i < 0 {
                -i
            } else if i >= n {
                2 * (n - 1) - i
            } else {
                i
            };
            x[j as usize]
        };
        let mut out = vec![0f32; N_MELS * FRAMES];
        let mut buf = vec![Complex::new(0f32, 0f32); N_FFT];
        for t in 0..FRAMES {
            let start = (t * STFT_HOP) as isize - half as isize;
            for (i, slot) in buf.iter_mut().enumerate() {
                *slot = Complex::new(at(start + i as isize) * self.window[i], 0.0);
            }
            self.fft.process(&mut buf);
            let power: Vec<f32> = buf[..=half].iter().map(|c| c.re * c.re + c.im * c.im).collect();
            for m in 0..N_MELS {
                let e: f32 = self.fb[m].iter().zip(&power).map(|(w, p)| w * p).sum();
                out[m * FRAMES + t] = e.max(1e-9).ln();
            }
        }
        for row in out.as_chunks_mut::<FRAMES>().0 {
            let mean = row.iter().sum::<f32>() / FRAMES as f32;
            row.iter_mut().for_each(|v| *v -= mean);
        }
        out
    }
}

struct Block {
    depth: Vec<f32>,
    point: Vec<f32>,
    bias: Vec<f32>,
    stride: usize,
}

struct Net {
    in_scale: Vec<f32>,
    in_shift: Vec<f32>,
    stem: Vec<f32>,
    stem_bias: Vec<f32>,
    blocks: Vec<Block>,
}

impl Net {
    fn new() -> Self {
        let all = floats(NET);
        let mut at = 0;
        let mut take = |n: usize| {
            let v = all[at..at + n].to_vec();
            at += n;
            v
        };
        let in_scale = take(N_MELS);
        let in_shift = take(N_MELS);
        let stem = take(DIM * N_MELS * STEM_KERNEL);
        let stem_bias = take(DIM);
        let blocks = STRIDES.iter().map(|&stride| Block { depth: take(DIM * BLOCK_KERNEL), point: take(DIM * DIM), bias: take(DIM), stride }).collect();
        assert_eq!(at, all.len(), "wake network size");
        Net { in_scale, in_shift, stem, stem_bias, blocks }
    }

    fn embed(&self, mel: &[f32]) -> Embedding {
        let x: Vec<f32> = mel.as_chunks::<FRAMES>().0.iter().enumerate().flat_map(|(m, row)| row.iter().map(move |v| v * self.in_scale[m] + self.in_shift[m])).collect();
        let (pad, stride) = (STEM_KERNEL / 2, 2);
        let mut len = (FRAMES + 2 * pad - STEM_KERNEL) / stride + 1;
        let mut h = vec![0f32; DIM * len];
        for o in 0..DIM {
            for t in 0..len {
                let mut acc = self.stem_bias[o];
                for i in 0..N_MELS {
                    let w = &self.stem[(o * N_MELS + i) * STEM_KERNEL..][..STEM_KERNEL];
                    let row = &x[i * FRAMES..][..FRAMES];
                    for (k, wk) in w.iter().enumerate() {
                        let p = (t * stride + k) as isize - pad as isize;
                        if p >= 0 && (p as usize) < FRAMES {
                            acc += wk * row[p as usize];
                        }
                    }
                }
                h[o * len + t] = acc.max(0.0);
            }
        }
        let pad = BLOCK_KERNEL / 2;
        for block in &self.blocks {
            let out_len = (len + 2 * pad - BLOCK_KERNEL) / block.stride + 1;
            let mut depth = vec![0f32; DIM * out_len];
            for c in 0..DIM {
                let w = &block.depth[c * BLOCK_KERNEL..][..BLOCK_KERNEL];
                let row = &h[c * len..][..len];
                for t in 0..out_len {
                    let mut acc = 0f32;
                    for (k, wk) in w.iter().enumerate() {
                        let p = (t * block.stride + k) as isize - pad as isize;
                        if p >= 0 && (p as usize) < len {
                            acc += wk * row[p as usize];
                        }
                    }
                    depth[c * out_len + t] = acc;
                }
            }
            let mut next = vec![0f32; DIM * out_len];
            for o in 0..DIM {
                let w = &block.point[o * DIM..][..DIM];
                for t in 0..out_len {
                    let mut acc = block.bias[o];
                    for (c, wc) in w.iter().enumerate() {
                        acc += wc * depth[c * out_len + t];
                    }
                    next[o * out_len + t] = acc.max(0.0);
                }
            }
            h = next;
            len = out_len;
        }
        let mut e = [0f32; DIM];
        for (c, slot) in e.iter_mut().enumerate() {
            *slot = h[c * len..][..len].iter().sum::<f32>() / len as f32;
        }
        e
    }
}

pub struct Detector {
    net: Net,
    features: Features,
    filter: Filter,
    buffer: Vec<f32>,
}

impl Detector {
    pub fn new() -> Self {
        Detector { net: Net::new(), features: Features::new(), filter: Filter::new(), buffer: vec![0.0; WINDOW] }
    }

    /// One hop of 16 kHz audio in; None when the last second is too quiet or not voice-like.
    pub fn step(&mut self, chunk: &[f32]) -> Option<Embedding> {
        let filtered = self.filter.run(chunk);
        let n = filtered.len().min(WINDOW);
        self.buffer.drain(..n);
        self.buffer.extend_from_slice(&filtered[filtered.len() - n..]);
        if !self.features.gate(&self.buffer) {
            return None;
        }
        let peak = self.buffer.iter().fold(0f32, |m, s| m.max(s.abs())).max(1e-9);
        let scale = if peak < 1e-6 { 1.0 } else { 10f32.powf(-3.0 / 20.0) / peak };
        let normalized: Vec<f32> = self.buffer.iter().map(|s| s * scale).collect();
        Some(self.net.embed(&self.features.log_mel(&normalized)))
    }
}

#[derive(Clone, Serialize, Deserialize)]
pub struct Head {
    pub weight: Vec<f32>,
    pub bias: f32,
    pub threshold: f32,
    #[serde(skip)]
    above: usize,
    #[serde(skip, default = "never")]
    last: f64,
}

fn never() -> f64 {
    -1e9
}

impl Head {
    pub fn generic() -> Self {
        let h = head_file();
        Head { weight: h.weight, bias: h.bias, threshold: h.threshold, above: 0, last: never() }
    }

    pub fn prob(&self, e: &Embedding) -> f32 {
        let z: f32 = self.weight.iter().zip(e).map(|(w, x)| w * x).sum::<f32>() + self.bias;
        1.0 / (1.0 + (-z).exp())
    }

    /// Heed's trigger rule: two hops over the threshold in a row, then a 0.7 s hold.
    pub fn update(&mut self, prob: Option<f32>, now: f64) -> bool {
        match prob {
            Some(p) if p > self.threshold => self.above += 1,
            _ => self.above = 0,
        }
        if self.above >= 2 && now - self.last > 0.7 {
            self.last = now;
            self.above = 0;
            return true;
        }
        false
    }

    pub fn reset(&mut self) {
        self.above = 0;
    }
}

pub struct Fit {
    pub head: Head,
    pub min_positive: f32,
    pub max_negative: f32,
    pub separated: bool,
}

/// Refits the last layer, pulled toward the generic weights so a handful of clips cannot undo the training.
pub fn fit_personal(base: &Head, user_pos: &[Embedding], user_neg: &[Embedding], best_per_try: &[Embedding]) -> Fit {
    let bank: Vec<(f32, Embedding)> = floats(BANK)
        .as_chunks::<{ DIM + 1 }>()
        .0
        .iter()
        .map(|r| {
            let mut e = [0f32; DIM];
            e.copy_from_slice(&r[1..]);
            (r[0], e)
        })
        .collect();
    let groups: [(f32, Vec<&Embedding>); 4] = [
        (1.0, bank.iter().filter(|(y, _)| *y > 0.5).map(|(_, e)| e).collect()),
        (0.0, bank.iter().filter(|(y, _)| *y < 0.5).map(|(_, e)| e).collect()),
        (1.0, user_pos.iter().collect()),
        (0.0, user_neg.iter().collect()),
    ];
    let mut data: Vec<(f32, f32, &Embedding)> = Vec::new();
    for (y, items) in &groups {
        let w = 1.0 / items.len().max(1) as f32;
        data.extend(items.iter().map(|e| (*y, w, *e)));
    }
    let total: f32 = data.iter().map(|(_, w, _)| w).sum();
    let (mut w, mut b) = (base.weight.clone(), base.bias);
    let (lambda, lr) = (0.01f32, 0.5f32);
    for _ in 0..1500 {
        let mut gw = [0f32; DIM];
        let mut gb = 0f32;
        for (y, c, e) in &data {
            let z: f32 = w.iter().zip(e.iter()).map(|(a, x)| a * x).sum::<f32>() + b;
            let err = (1.0 / (1.0 + (-z).exp()) - y) * c / total;
            for (g, x) in gw.iter_mut().zip(e.iter()) {
                *g += err * x;
            }
            gb += err;
        }
        for i in 0..DIM {
            w[i] -= lr * (gw[i] + 2.0 * lambda * (w[i] - base.weight[i]));
        }
        b -= lr * gb;
    }
    let mut head = Head { weight: w, bias: b, threshold: base.threshold, above: 0, last: never() };
    let min_positive = best_per_try.iter().map(|e| head.prob(e)).fold(1.0, f32::min);
    let max_negative = user_neg.iter().map(|e| head.prob(e)).fold(0.0, f32::max);
    let separated = min_positive > max_negative;
    if separated {
        head.threshold = ((min_positive + max_negative) / 2.0).clamp(0.5, 0.95);
    }
    Fit { head, min_positive, max_negative, separated }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Deserialize)]
    struct Fixture {
        filtered: Vec<f32>,
        mel_rows: Vec<f32>,
        mel_points: Vec<f32>,
        embedding: Vec<f32>,
        logit: f32,
    }

    fn fixture() -> Fixture {
        serde_json::from_str(include_str!("model/fixture.json")).unwrap()
    }

    fn tone() -> Vec<f32> {
        (0..RATE)
            .map(|i| {
                let t = i as f64 / RATE as f64;
                (0.3 * (2.0 * PI * 440.0 * t).sin() + 0.2 * (2.0 * PI * 1234.0 * t + 0.5).sin() + 0.05 * (2.0 * PI * 60.0 * t).sin()) as f32
            })
            .collect()
    }

    #[test]
    fn filter_matches_scipy() {
        let out = Filter::new().run(&tone());
        for (i, want) in [0, 1, 2, 100, 1000, 8000, 15999].into_iter().zip(fixture().filtered) {
            assert!((out[i] - want).abs() < 1e-5, "sample {i}: {} vs {want}", out[i]);
        }
    }

    #[test]
    fn log_mel_matches_torch() {
        let f = fixture();
        let mel = Features::new().log_mel(&tone());
        for (m, want) in f.mel_rows.iter().enumerate() {
            let got: f32 = mel[m * FRAMES..][..FRAMES].iter().sum();
            assert!((got - want).abs() < 2e-2, "row {m}: {got} vs {want}");
        }
        for ((m, t), want) in [(0, 0), (5, 50), (20, 100), (39, 7)].into_iter().zip(f.mel_points) {
            assert!((mel[m * FRAMES + t] - want).abs() < 1e-3, "mel[{m}][{t}]: {} vs {want}", mel[m * FRAMES + t]);
        }
    }

    #[test]
    fn network_matches_torch() {
        let f = fixture();
        let mel: Vec<f32> = (0..N_MELS).flat_map(|m| (0..FRAMES).map(move |t| ((0.1 * m as f64 + 0.07 * t as f64).sin() * 3.0) as f32)).collect();
        let e = Net::new().embed(&mel);
        for (i, (got, want)) in e.iter().zip(&f.embedding).enumerate() {
            assert!((got - want).abs() < 1e-4, "embedding[{i}]: {got} vs {want}");
        }
        let h = Head::generic();
        let logit: f32 = h.weight.iter().zip(&e).map(|(w, x)| w * x).sum::<f32>() + h.bias;
        assert!((logit - f.logit).abs() < 1e-3, "logit {logit} vs {}", f.logit);
    }

    /// `WAKE_REF=<folder with probs.json and *.f32 from heed> cargo test -p pob-redux --lib wake -- --ignored`
    #[test]
    #[ignore]
    fn streaming_matches_heed() {
        let dir = std::env::var("WAKE_REF").unwrap();
        let refs: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(format!("{dir}/probs.json")).unwrap()).unwrap();
        for (name, expected) in refs.as_object().unwrap() {
            let audio = floats(&std::fs::read(format!("{dir}/{}.f32", name.trim_end_matches(".wav"))).unwrap());
            let expected = expected.as_array().unwrap();
            let (mut detector, mut head) = (Detector::new(), Head::generic());
            let (mut worst, mut triggers) = (0f32, 0);
            let started = std::time::Instant::now();
            for (i, start) in (0..(audio.len().saturating_sub(HOP) + 1).max(1)).step_by(HOP).enumerate() {
                let prob = detector.step(&audio[start..(start + HOP).min(audio.len())]).map(|e| head.prob(&e));
                triggers += head.update(prob, i as f64 * 0.1) as usize;
                assert_eq!(prob.is_none(), expected[i]["gated"].as_bool().unwrap(), "{name} step {i} gate");
                worst = worst.max((prob.unwrap_or(0.0) - expected[i]["prob"].as_f64().unwrap() as f32).abs());
            }
            let want = expected.iter().filter(|e| e["triggered"].as_bool().unwrap()).count();
            println!("{name}: worst diff {worst:.1e}, triggers {triggers}/{want}, {:.2} ms per hop", started.elapsed().as_secs_f64() * 1000.0 / expected.len() as f64);
            assert!(worst < 1e-4 && triggers == want);
        }
    }

    #[test]
    fn generic_head_separates_the_examples() {
        let h = Head::generic();
        let bank = floats(BANK);
        let (mut pos, mut neg) = (0, 0);
        for row in bank.chunks_exact(DIM + 1) {
            let mut e = [0f32; DIM];
            e.copy_from_slice(&row[1..]);
            let p = h.prob(&e);
            if row[0] > 0.5 && p <= h.threshold {
                pos += 1;
            }
            if row[0] < 0.5 && p > h.threshold {
                neg += 1;
            }
        }
        assert_eq!((pos, neg), (0, 0));
    }
}
