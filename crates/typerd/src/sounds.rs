//! Turns a pack into ready-to-mix mono buffers at the output sample rate.

use std::collections::HashMap;
use std::fs::File;
use std::path::{Path, PathBuf};

use anyhow::{Context, anyhow};
use symphonia::core::audio::SampleBuffer;
use symphonia::core::codecs::DecoderOptions;
use symphonia::core::errors::Error as SymphoniaError;
use symphonia::core::formats::FormatOptions;
use symphonia::core::io::MediaSourceStream;
use symphonia::core::meta::MetadataOptions;
use symphonia::core::probe::Hint;
use typer_core::keycodes::{KEY_BACKSPACE, KEY_ENTER, KEY_SPACE, KEY_TAB};
use typer_core::pack::{self, BUILTIN_PREFIX, KeySound, Manifest};

use crate::audio::Sample;

pub struct Sounds {
    pub id: String,
    pub name: String,
    keys: HashMap<u16, Sample>,
    /// Used for keys the pack doesn't define; picked by key code so a key always sounds the same.
    fallback: Vec<Sample>,
}

impl Sounds {
    pub fn load(id: &str, rate: u32) -> anyhow::Result<Self> {
        if let Some(kind) = id.strip_prefix(BUILTIN_PREFIX) {
            let name = pack::BUILTINS
                .iter()
                .find(|(builtin, _)| *builtin == id)
                .map(|(_, name)| *name)
                .ok_or_else(|| anyhow!("unknown built-in pack {id}"))?;
            return Ok(synth::pack(id, name, kind, rate));
        }
        load_mechvibes(&Manifest::load(&pack::user_pack_dir(id))?, rate)
    }

    pub fn for_key(&self, code: u16) -> Option<Sample> {
        self.keys
            .get(&code)
            .or_else(|| self.fallback.get(usize::from(code) % self.fallback.len().max(1)))
            .cloned()
    }
}

fn load_mechvibes(manifest: &Manifest, rate: u32) -> anyhow::Result<Sounds> {
    let mut keys = HashMap::new();
    let mut file_cache: HashMap<PathBuf, Sample> = HashMap::new();
    let sprite = manifest.sprite.as_deref().map(decode).transpose()?;

    for (&code, sound) in &manifest.keys {
        let sample = match sound {
            KeySound::Slice { start_ms, duration_ms } => {
                let (pcm, src_rate) = sprite.as_ref().context("single pack without sprite")?;
                let ms_to_frame = |ms: f64| ((ms / 1000.0) * f64::from(*src_rate)) as usize;
                let start = ms_to_frame(*start_ms).min(pcm.len());
                let end = (start + ms_to_frame(*duration_ms)).min(pcm.len());
                finish(&pcm[start..end], *src_rate, rate)
            }
            KeySound::File(path) => match file_cache.get(path) {
                Some(s) => Some(s.clone()),
                None => match decode(path) {
                    Ok((pcm, src_rate)) => {
                        let s = finish(&pcm, src_rate, rate);
                        if let Some(s) = &s {
                            file_cache.insert(path.clone(), s.clone());
                        }
                        s
                    }
                    Err(e) => {
                        eprintln!("typerd: skipping {}: {e:#}", path.display());
                        None
                    }
                },
            },
        };
        if let Some(sample) = sample {
            keys.insert(code, sample);
        }
    }
    if keys.is_empty() {
        return Err(anyhow!("pack {} has no playable sounds", manifest.id));
    }

    // Letters (Q..P, A..L, Z..M rows) make the most natural fallback.
    let letters = (16..=25).chain(30..=38).chain(44..=50);
    let mut fallback: Vec<Sample> = letters.filter_map(|c| keys.get(&c).cloned()).collect();
    if fallback.is_empty() {
        fallback = keys.values().cloned().collect();
    }
    Ok(Sounds { id: manifest.id.clone(), name: manifest.name.clone(), keys, fallback })
}

/// Decode any supported file to mono f32 at its native rate.
fn decode(path: &Path) -> anyhow::Result<(Vec<f32>, u32)> {
    let file = File::open(path).with_context(|| format!("opening {}", path.display()))?;
    let mss = MediaSourceStream::new(Box::new(file), Default::default());
    let mut hint = Hint::new();
    if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
        hint.with_extension(ext);
    }
    let probed = symphonia::default::get_probe().format(
        &hint,
        mss,
        &FormatOptions::default(),
        &MetadataOptions::default(),
    )?;
    let mut format = probed.format;
    let track = format.default_track().context("no audio track")?;
    let track_id = track.id;
    let mut decoder = symphonia::default::get_codecs().make(&track.codec_params, &DecoderOptions::default())?;
    let mut rate = track.codec_params.sample_rate.unwrap_or(44_100);

    let mut pcm = Vec::new();
    let mut buf: Option<SampleBuffer<f32>> = None;
    loop {
        let packet = match format.next_packet() {
            Ok(p) => p,
            Err(SymphoniaError::IoError(e)) if e.kind() == std::io::ErrorKind::UnexpectedEof => break,
            Err(e) => return Err(e.into()),
        };
        if packet.track_id() != track_id {
            continue;
        }
        let decoded = match decoder.decode(&packet) {
            Ok(d) => d,
            Err(SymphoniaError::DecodeError(_)) => continue,
            Err(e) => return Err(e.into()),
        };
        let spec = *decoded.spec();
        rate = spec.rate;
        let channels = spec.channels.count().max(1);
        let sb = buf.get_or_insert_with(|| SampleBuffer::new(decoded.capacity() as u64, spec));
        if sb.capacity() < decoded.capacity() * channels {
            *sb = SampleBuffer::new(decoded.capacity() as u64, spec);
        }
        sb.copy_interleaved_ref(decoded);
        pcm.extend(sb.samples().chunks(channels).map(|f| f.iter().sum::<f32>() / channels as f32));
    }
    Ok((pcm, rate))
}

/// Resample (linear is plenty for clicks), add a short fade-out and freeze into a shared buffer.
fn finish(pcm: &[f32], from: u32, to: u32) -> Option<Sample> {
    if pcm.is_empty() {
        return None;
    }
    let mut out: Vec<f32> = if from == to {
        pcm.to_vec()
    } else {
        let ratio = f64::from(from) / f64::from(to);
        let len = ((pcm.len() as f64) / ratio) as usize;
        (0..len)
            .map(|i| {
                let pos = i as f64 * ratio;
                let idx = pos as usize;
                let frac = (pos - idx as f64) as f32;
                let a = pcm[idx.min(pcm.len() - 1)];
                let b = pcm[(idx + 1).min(pcm.len() - 1)];
                a + (b - a) * frac
            })
            .collect()
    };
    let fade = (to as usize / 500).min(out.len()); // 2 ms
    let n = out.len();
    for (i, s) in out[n - fade..].iter_mut().enumerate() {
        *s *= 1.0 - (i as f32 / fade as f32);
    }
    (!out.is_empty()).then(|| out.into())
}

/// Procedural packs so typer works out of the box without shipping third-party audio.
mod synth {
    use super::*;

    struct Rng(u32);
    impl Rng {
        fn noise(&mut self) -> f32 {
            self.0 ^= self.0 << 13;
            self.0 ^= self.0 >> 17;
            self.0 ^= self.0 << 5;
            (self.0 as f32 / u32::MAX as f32) * 2.0 - 1.0
        }
    }

    /// Damped noise transient plus a damped sine body.
    fn hit(rate: u32, seed: u32, ms: f32, noise_tau: f32, tone_hz: f32, tone_tau: f32, tone_amp: f32) -> Sample {
        let sr = rate as f32;
        let len = (sr * ms / 1000.0) as usize;
        let mut rng = Rng(seed.wrapping_mul(2_654_435_761).max(1));
        let mut lp = 0.0f32;
        let out: Vec<f32> = (0..len)
            .map(|i| {
                let t = i as f32 / sr;
                // One-pole lowpass takes the hiss out of the white noise.
                lp += 0.35 * (rng.noise() - lp);
                let click = lp * (-t / noise_tau).exp();
                let body = (std::f32::consts::TAU * tone_hz * t).sin() * (-t / tone_tau).exp() * tone_amp;
                (click + body) * 0.8
            })
            .collect();
        out.into()
    }

    pub fn pack(id: &str, name: &str, kind: &str, rate: u32) -> Sounds {
        // (tone_hz, tone_tau, tone_amp, noise_tau) per sound family
        let (base_hz, tau, amp, noise_tau) = match kind {
            "thock" => (190.0, 0.022, 0.9, 0.0025),
            _ => (2300.0, 0.006, 0.35, 0.004),
        };
        let mut keys = HashMap::new();
        let special = [
            // (key, pitch, length, gain)
            (KEY_SPACE, 0.62, 1.6, 1.15),
            (KEY_ENTER, 0.78, 1.4, 1.2),
            (KEY_BACKSPACE, 0.88, 1.2, 1.05),
            (KEY_TAB, 0.92, 1.1, 1.0),
        ];
        for (i, (code, pitch, len, gain)) in special.into_iter().enumerate() {
            keys.insert(
                code,
                hit(rate, 97 + i as u32, 70.0 * len, noise_tau * len, base_hz * pitch, tau * len, amp * gain),
            );
        }
        // A handful of slightly detuned variants so fast typing doesn't sound robotic.
        let fallback = (0..6)
            .map(|v| {
                let detune = 1.0 + (v as f32 - 2.5) * 0.025;
                hit(rate, 7 + v, 60.0, noise_tau, base_hz * detune, tau, amp)
            })
            .collect();
        Sounds { id: id.into(), name: name.into(), keys, fallback }
    }
}
