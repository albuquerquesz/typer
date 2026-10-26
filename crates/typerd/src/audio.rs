//! Output stream and a tiny realtime mixer. Key presses push voices through a
//! lock-free ring buffer; the audio callback never allocates or locks.

use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Arc, Mutex, mpsc};
use std::thread;

use anyhow::{Context, anyhow};
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{FromSample, SampleFormat, SizedSample, StreamConfig};

const MAX_VOICES: usize = 16;

pub type Sample = Arc<[f32]>;

struct Voice {
    buf: Sample,
    pos: usize,
    gain: f32,
}

enum Ctl {
    Play,
    Pause,
}

pub struct Audio {
    producer: Mutex<rtrb::Producer<Voice>>,
    volume: Arc<AtomicU32>,
    ctl: mpsc::Sender<Ctl>,
    pub sample_rate: u32,
}

impl Audio {
    /// Open the default output device on a dedicated thread (cpal streams are `!Send`).
    pub fn start(volume: u8, playing: bool) -> anyhow::Result<Self> {
        let (producer, consumer) = rtrb::RingBuffer::new(64);
        let volume_bits = Arc::new(AtomicU32::new(gain_for(volume).to_bits()));
        let (ctl_tx, ctl_rx) = mpsc::channel();
        let (ready_tx, ready_rx) = mpsc::channel();

        let vol = volume_bits.clone();
        thread::Builder::new().name("audio".into()).spawn(move || {
            let stream = match open_stream(consumer, vol) {
                Ok((stream, rate)) => {
                    let _ = ready_tx.send(Ok(rate));
                    stream
                }
                Err(e) => {
                    let _ = ready_tx.send(Err(e));
                    return;
                }
            };
            if playing {
                let _ = stream.play();
            }
            for msg in ctl_rx {
                let result = match msg {
                    Ctl::Play => stream.play().map_err(|e| e.to_string()),
                    Ctl::Pause => stream.pause().map_err(|e| e.to_string()),
                };
                if let Err(e) = result {
                    eprintln!("typerd: audio stream control failed: {e}");
                }
            }
        })?;

        let sample_rate = ready_rx.recv().context("audio thread died")??;
        Ok(Self { producer: Mutex::new(producer), volume: volume_bits, ctl: ctl_tx, sample_rate })
    }

    pub fn trigger(&self, buf: Sample, gain: f32) {
        // A full ring means 64 pending voices; dropping one press is fine.
        let _ = self.producer.lock().unwrap().push(Voice { buf, pos: 0, gain });
    }

    pub fn set_volume(&self, volume: u8) {
        self.volume.store(gain_for(volume).to_bits(), Ordering::Relaxed);
    }

    pub fn set_playing(&self, playing: bool) {
        let _ = self.ctl.send(if playing { Ctl::Play } else { Ctl::Pause });
    }
}

/// Perceptual curve: slider 50% ≈ -12 dB instead of -6 dB.
fn gain_for(volume: u8) -> f32 {
    let v = f32::from(volume.min(100)) / 100.0;
    v * v
}

fn open_stream(
    consumer: rtrb::Consumer<Voice>,
    volume: Arc<AtomicU32>,
) -> anyhow::Result<(cpal::Stream, u32)> {
    let device = cpal::default_host()
        .default_output_device()
        .ok_or_else(|| anyhow!("no default audio output device"))?;
    let supported = device.default_output_config()?;
    let format = supported.sample_format();
    let config: StreamConfig = supported.into();
    let rate = config.sample_rate.0;

    let stream = match format {
        SampleFormat::F32 => build::<f32>(&device, &config, consumer, volume),
        SampleFormat::I16 => build::<i16>(&device, &config, consumer, volume),
        SampleFormat::I32 => build::<i32>(&device, &config, consumer, volume),
        SampleFormat::U16 => build::<u16>(&device, &config, consumer, volume),
        other => return Err(anyhow!("unsupported sample format {other}")),
    }?;
    Ok((stream, rate))
}

fn build<T: SizedSample + FromSample<f32>>(
    device: &cpal::Device,
    config: &StreamConfig,
    mut consumer: rtrb::Consumer<Voice>,
    volume: Arc<AtomicU32>,
) -> anyhow::Result<cpal::Stream> {
    let channels = usize::from(config.channels);
    let mut voices: [Option<Voice>; MAX_VOICES] = Default::default();

    let stream = device.build_output_stream(
        config,
        move |data: &mut [T], _| {
            while let Ok(voice) = consumer.pop() {
                let slot = voices.iter().position(Option::is_none).unwrap_or_else(|| {
                    // Steal the voice that has played the longest.
                    let mut oldest = 0;
                    for (i, v) in voices.iter().enumerate() {
                        if v.as_ref().map_or(0, |v| v.pos) > voices[oldest].as_ref().map_or(0, |v| v.pos) {
                            oldest = i;
                        }
                    }
                    oldest
                });
                voices[slot] = Some(voice);
            }

            let master = f32::from_bits(volume.load(Ordering::Relaxed));
            for frame in data.chunks_mut(channels) {
                let mut mix = 0.0;
                for slot in voices.iter_mut() {
                    if let Some(v) = slot {
                        mix += v.buf[v.pos] * v.gain;
                        v.pos += 1;
                        if v.pos >= v.buf.len() {
                            *slot = None;
                        }
                    }
                }
                let out = T::from_sample((mix * master).clamp(-1.0, 1.0));
                frame.fill(out);
            }
        },
        |e| eprintln!("typerd: audio stream error: {e}"),
        None,
    )?;
    Ok(stream)
}
