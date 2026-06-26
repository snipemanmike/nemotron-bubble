use anyhow::{anyhow, Result};
use base64::{engine::general_purpose::STANDARD, Engine as _};
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use crossbeam_channel::{bounded, Receiver, Sender};
use parakeet_rs::{Nemotron, NemotronMode};
use serde::{Deserialize, Serialize};
use std::env;
use std::io::{self, BufRead, Write};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

const NEMOTRON_SAMPLE_RATE: f64 = 16_000.0;
const NEMOTRON_CHUNK_SIZE: usize = 8_960; // 560 ms at 16 kHz.
const SILENCE_AUTO_STOP_SECS: u64 = 15;
const VOICE_RMS_THRESHOLD: f32 = 0.02;

#[derive(Clone, Debug)]
enum EngineCommand {
    Preload,
    Start,
    RawAudio { samples: Vec<f32>, sample_rate: f64 },
    Audio(Vec<f32>),
    Stop { fast: bool, auto: bool },
    Shutdown,
}

#[derive(Debug, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum ClientCommand {
    Preload,
    Start,
    Audio { sample_rate: f64, data: String },
    Stop { fast: Option<bool> },
    Shutdown,
}

#[derive(Clone, Debug, Serialize)]
struct EngineEvent {
    #[serde(rename = "type")]
    kind: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    message: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    text: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    delta: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    path: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    level: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    recording: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    auto: Option<bool>,
}

impl EngineEvent {
    fn status(message: impl Into<String>) -> Self {
        Self {
            kind: "status",
            message: Some(message.into()),
            text: None,
            delta: None,
            path: None,
            level: None,
            recording: None,
            auto: None,
        }
    }

    fn error(message: impl Into<String>) -> Self {
        Self {
            kind: "error",
            message: Some(message.into()),
            text: None,
            delta: None,
            path: None,
            level: None,
            recording: None,
            auto: None,
        }
    }

    fn model_dir(path: impl Into<String>) -> Self {
        Self {
            kind: "model_dir",
            message: None,
            text: None,
            delta: None,
            path: Some(path.into()),
            level: None,
            recording: None,
            auto: None,
        }
    }

    fn transcript(text: impl Into<String>, delta: impl Into<String>) -> Self {
        Self {
            kind: "transcript",
            message: None,
            text: Some(text.into()),
            delta: Some(delta.into()),
            path: None,
            level: None,
            recording: None,
            auto: None,
        }
    }

    fn final_text(text: impl Into<String>, auto: bool) -> Self {
        Self {
            kind: "final",
            message: None,
            text: Some(text.into()),
            delta: None,
            path: None,
            level: None,
            recording: Some(false),
            auto: Some(auto),
        }
    }

    fn level(level: f32) -> Self {
        Self {
            kind: "level",
            message: None,
            text: None,
            delta: None,
            path: None,
            level: Some(level),
            recording: None,
            auto: None,
        }
    }

    fn recording(recording: bool) -> Self {
        Self {
            kind: "recording",
            message: None,
            text: None,
            delta: None,
            path: None,
            level: None,
            recording: Some(recording),
            auto: None,
        }
    }
}

fn main() -> Result<()> {
    if env::args().any(|arg| arg == "--self-test") {
        let (mut model, model_dir) = load_nemotron()?;
        model.reset();
        let mode = match model.mode() {
            NemotronMode::EnglishOnly => "English-only",
            NemotronMode::Multilingual => "Multilingual",
        };
        let _ = model.transcribe_chunk(&vec![0.0; NEMOTRON_CHUNK_SIZE])?;
        println!("Nemotron loaded: {mode}");
        println!("Model dir: {}", model_dir.display());
        return Ok(());
    }

    let (command_tx, command_rx) = bounded::<EngineCommand>(512);
    let (event_tx, event_rx) = bounded::<EngineEvent>(512);
    let recording = Arc::new(AtomicBool::new(false));
    let last_voice = Arc::new(Mutex::new(Instant::now()));

    thread::spawn(move || write_events(event_rx));
    spawn_command_reader(command_tx.clone());

    let stdin_audio = env::args().any(|arg| arg == "--stdin-audio");
    let _keep_audio_alive = if stdin_audio {
        let _ = event_tx.send(EngineEvent::status(
            "Ready. Mic is handled by Nemotron Bubble.",
        ));
        None
    } else {
        Some(
            match start_audio_capture(
                recording.clone(),
                command_tx.clone(),
                event_tx.clone(),
                last_voice.clone(),
            ) {
                Ok(stream) => stream,
                Err(err) => {
                    let _ = event_tx.send(EngineEvent::error(format!("Microphone error: {err:#}")));
                    return Err(err);
                }
            },
        )
    };

    let _ = event_tx.send(EngineEvent::status("Ready. Press Ctrl-Space to start."));
    run_engine(command_rx, command_tx, event_tx, recording, last_voice);
    Ok(())
}

fn spawn_command_reader(tx: Sender<EngineCommand>) {
    thread::spawn(move || {
        let stdin = io::stdin();
        for line in stdin.lock().lines() {
            let Ok(line) = line else { break };
            if line.trim().is_empty() {
                continue;
            }

            let command = match serde_json::from_str::<ClientCommand>(&line) {
                Ok(ClientCommand::Preload) => EngineCommand::Preload,
                Ok(ClientCommand::Start) => EngineCommand::Start,
                Ok(ClientCommand::Audio { sample_rate, data }) => {
                    let Ok(samples) = decode_f32_base64(&data) else {
                        continue;
                    };
                    EngineCommand::RawAudio {
                        samples,
                        sample_rate,
                    }
                }
                Ok(ClientCommand::Stop { fast }) => EngineCommand::Stop {
                    fast: fast.unwrap_or(false),
                    auto: false,
                },
                Ok(ClientCommand::Shutdown) => EngineCommand::Shutdown,
                Err(_) => continue,
            };

            if tx.send(command).is_err() {
                break;
            }
        }

        let _ = tx.send(EngineCommand::Shutdown);
    });
}

fn decode_f32_base64(data: &str) -> Result<Vec<f32>> {
    let bytes = STANDARD.decode(data)?;
    if bytes.len() % std::mem::size_of::<f32>() != 0 {
        return Err(anyhow!("audio payload was not f32-aligned"));
    }

    let mut samples = Vec::with_capacity(bytes.len() / 4);
    for chunk in bytes.chunks_exact(4) {
        samples.push(f32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]));
    }
    Ok(samples)
}

fn write_events(rx: Receiver<EngineEvent>) {
    let stdout = io::stdout();
    let mut stdout = stdout.lock();

    for event in rx {
        if serde_json::to_writer(&mut stdout, &event).is_err() {
            break;
        }
        if stdout.write_all(b"\n").is_err() || stdout.flush().is_err() {
            break;
        }
    }
}

fn run_engine(
    rx: Receiver<EngineCommand>,
    commands: Sender<EngineCommand>,
    events: Sender<EngineEvent>,
    recording: Arc<AtomicBool>,
    last_voice: Arc<Mutex<Instant>>,
) {
    let mut model: Option<Nemotron> = None;
    let mut active = false;
    let mut audio_buffer = Vec::<f32>::with_capacity(NEMOTRON_CHUNK_SIZE * 2);
    let mut live_transcript = String::new();
    let mut stdin_audio = StdinAudioProcessor::new(
        events.clone(),
        recording.clone(),
        last_voice.clone(),
        commands.clone(),
    );

    loop {
        match rx.recv() {
            Ok(EngineCommand::Preload) => {
                if model.is_some() {
                    continue;
                }
                let _ = events.send(EngineEvent::status("Loading Nemotron..."));
                match load_nemotron() {
                    Ok((loaded, path)) => {
                        let display_path = path.display().to_string();
                        model = Some(loaded);
                        let _ = events.send(EngineEvent::status("Ready. Nemotron loaded."));
                        let _ = events.send(EngineEvent::model_dir(display_path));
                    }
                    Err(err) => {
                        let _ = events.send(EngineEvent::error(format!("{err:#}")));
                        let _ = events.send(EngineEvent::model_dir(expected_model_hint()));
                    }
                }
            }
            Ok(EngineCommand::Start) => {
                audio_buffer.clear();
                live_transcript.clear();
                stdin_audio.reset();
                recording.store(true, Ordering::SeqCst);
                if let Ok(mut last_voice) = last_voice.lock() {
                    *last_voice = Instant::now();
                }

                if model.is_none() {
                    let _ = events.send(EngineEvent::status("Loading Nemotron model..."));
                    match load_nemotron() {
                        Ok((loaded, path)) => {
                            let display_path = path.display().to_string();
                            model = Some(loaded);
                            let _ = events.send(EngineEvent::model_dir(display_path));
                        }
                        Err(err) => {
                            recording.store(false, Ordering::SeqCst);
                            let _ = events.send(EngineEvent::recording(false));
                            let _ = events.send(EngineEvent::error(format!("{err:#}")));
                            let _ = events.send(EngineEvent::model_dir(expected_model_hint()));
                            continue;
                        }
                    }
                }

                if let Some(model) = model.as_mut() {
                    model.reset();
                    active = true;
                    let _ = events.send(EngineEvent::recording(true));
                    let _ = events.send(EngineEvent::status("Listening..."));
                    let _ = events.send(EngineEvent::transcript("", ""));
                }
            }
            Ok(EngineCommand::RawAudio {
                samples,
                sample_rate,
            }) => {
                if !active {
                    continue;
                }
                let samples = stdin_audio.process(sample_rate, &samples);
                if samples.is_empty() {
                    continue;
                }
                transcribe_audio(
                    samples,
                    model.as_mut(),
                    &mut audio_buffer,
                    &mut live_transcript,
                    &events,
                    &recording,
                    &mut active,
                );
            }
            Ok(EngineCommand::Audio(samples)) => {
                if !active {
                    continue;
                }
                transcribe_audio(
                    samples,
                    model.as_mut(),
                    &mut audio_buffer,
                    &mut live_transcript,
                    &events,
                    &recording,
                    &mut active,
                );
            }
            Ok(EngineCommand::Stop { fast, auto }) => {
                recording.store(false, Ordering::SeqCst);
                if active {
                    active = false;
                    if let Some(model) = model.as_mut() {
                        if !audio_buffer.is_empty() {
                            audio_buffer.resize(NEMOTRON_CHUNK_SIZE, 0.0);
                            if let Ok(text) = model.transcribe_chunk(&audio_buffer) {
                                if !text.is_empty() {
                                    live_transcript.push_str(&text);
                                    let _ = events.send(EngineEvent::transcript(
                                        live_transcript.clone(),
                                        text,
                                    ));
                                }
                            }
                            audio_buffer.clear();
                        }

                        let flush_passes = if fast { 0 } else { 3 };
                        for _ in 0..flush_passes {
                            if let Ok(text) =
                                model.transcribe_chunk(&vec![0.0; NEMOTRON_CHUNK_SIZE])
                            {
                                if !text.is_empty() {
                                    live_transcript.push_str(&text);
                                    let _ = events.send(EngineEvent::transcript(
                                        live_transcript.clone(),
                                        text,
                                    ));
                                }
                            }
                        }

                        let final_text = model.get_transcript();
                        let _ = events.send(EngineEvent::final_text(final_text, auto));
                        let _ = events.send(EngineEvent::status(if auto {
                            "Stopped after silence."
                        } else {
                            "Stopped."
                        }));
                    }
                } else {
                    let _ = events.send(EngineEvent::recording(false));
                    let _ = events.send(EngineEvent::status("Ready. Press Ctrl-Space to start."));
                }
            }
            Ok(EngineCommand::Shutdown) | Err(_) => break,
        }
    }
}

fn transcribe_audio(
    samples: Vec<f32>,
    model: Option<&mut Nemotron>,
    audio_buffer: &mut Vec<f32>,
    live_transcript: &mut String,
    events: &Sender<EngineEvent>,
    recording: &Arc<AtomicBool>,
    active: &mut bool,
) {
    let Some(model) = model else {
        return;
    };

    audio_buffer.extend(samples);
    while audio_buffer.len() >= NEMOTRON_CHUNK_SIZE {
        let chunk: Vec<f32> = audio_buffer.drain(..NEMOTRON_CHUNK_SIZE).collect();
        match model.transcribe_chunk(&chunk) {
            Ok(text) if !text.is_empty() => {
                live_transcript.push_str(&text);
                let _ = events.send(EngineEvent::transcript(live_transcript.clone(), text));
            }
            Ok(_) => {}
            Err(err) => {
                *active = false;
                recording.store(false, Ordering::SeqCst);
                let _ = events.send(EngineEvent::recording(false));
                let _ = events.send(EngineEvent::error(format!("ASR error: {err:#}")));
            }
        }
    }
}

struct StdinAudioProcessor {
    tx: Sender<EngineCommand>,
    events: Sender<EngineEvent>,
    recording: Arc<AtomicBool>,
    last_voice: Arc<Mutex<Instant>>,
    resampler: LinearResampler,
    source_rate: f64,
}

impl StdinAudioProcessor {
    fn new(
        events: Sender<EngineEvent>,
        recording: Arc<AtomicBool>,
        last_voice: Arc<Mutex<Instant>>,
        tx: Sender<EngineCommand>,
    ) -> Self {
        Self {
            tx,
            events,
            recording,
            last_voice,
            resampler: LinearResampler::new(NEMOTRON_SAMPLE_RATE, NEMOTRON_SAMPLE_RATE),
            source_rate: NEMOTRON_SAMPLE_RATE,
        }
    }

    fn reset(&mut self) {
        self.resampler.reset();
        if let Ok(mut last_voice) = self.last_voice.lock() {
            *last_voice = Instant::now();
        }
    }

    fn process(&mut self, sample_rate: f64, mono: &[f32]) -> Vec<f32> {
        if !self.recording.load(Ordering::Relaxed) || mono.is_empty() {
            return Vec::new();
        }

        if (self.source_rate - sample_rate).abs() > f64::EPSILON {
            self.source_rate = sample_rate;
            self.resampler = LinearResampler::new(sample_rate, NEMOTRON_SAMPLE_RATE);
        }

        self.report_level_and_silence(mono);

        let mut out = Vec::new();
        self.resampler.push(mono, &mut out);
        out
    }

    fn report_level_and_silence(&mut self, mono: &[f32]) {
        const NOISE_FLOOR: f32 = 0.006;
        const GAIN: f32 = 14.0;
        const SHAPE: f32 = 0.6;

        let rms = (mono.iter().map(|v| v * v).sum::<f32>() / mono.len() as f32).sqrt();
        let norm = ((rms - NOISE_FLOOR) * GAIN).clamp(0.0, 1.0);
        let level = norm.powf(SHAPE);
        let _ = self.events.try_send(EngineEvent::level(level));

        if rms > VOICE_RMS_THRESHOLD {
            if let Ok(mut last_voice) = self.last_voice.lock() {
                *last_voice = Instant::now();
            }
        } else if self.silence_elapsed() >= Duration::from_secs(SILENCE_AUTO_STOP_SECS)
            && self.recording.swap(false, Ordering::SeqCst)
        {
            let _ = self.tx.try_send(EngineCommand::Stop {
                fast: false,
                auto: true,
            });
        }
    }

    fn silence_elapsed(&self) -> Duration {
        self.last_voice
            .lock()
            .map(|last_voice| last_voice.elapsed())
            .unwrap_or_default()
    }
}

fn load_nemotron() -> Result<(Nemotron, PathBuf)> {
    let model_dir = find_model_dir()?;
    let mut model = Nemotron::from_pretrained(&model_dir, None)
        .map_err(|err| anyhow!("failed to load {}: {err}", model_dir.display()))?;

    if model.mode() == NemotronMode::Multilingual {
        model
            .set_target_lang("auto")
            .map_err(|err| anyhow!("failed to set multilingual auto language: {err}"))?;
    }

    Ok((model, model_dir))
}

fn find_model_dir() -> Result<PathBuf> {
    let mut candidates = Vec::<PathBuf>::new();

    if let Ok(path) = env::var("NEMOTRON_MODEL_DIR") {
        candidates.push(PathBuf::from(path));
    }

    if let Ok(cwd) = env::current_dir() {
        candidates.push(cwd.join("models").join("nemotron"));
        candidates.push(cwd.join("models").join("nemotron_multi"));
    }

    if let Some(home) = home_dir() {
        candidates.push(
            home.join("Library")
                .join("Application Support")
                .join("NemotronBubble")
                .join("models")
                .join("nemotron"),
        );
        candidates.push(
            home.join("Library")
                .join("Application Support")
                .join("NemotronBubble")
                .join("models")
                .join("nemotron_multi"),
        );
    }

    if let Ok(exe) = env::current_exe() {
        let mut dir = exe.parent();
        for _ in 0..10 {
            let Some(d) = dir else { break };
            candidates.push(d.join("models").join("nemotron"));
            candidates.push(d.join("models").join("nemotron_multi"));
            dir = d.parent();
        }
    }

    for candidate in &candidates {
        if candidate.join("encoder.onnx").exists()
            && candidate.join("decoder_joint.onnx").exists()
            && candidate.join("tokenizer.model").exists()
        {
            return Ok(candidate.clone());
        }
    }

    Err(anyhow!(
        "Expected Nemotron ONNX files in one of these folders:\n{}",
        candidates
            .iter()
            .map(|p| format!("  - {}", p.display()))
            .collect::<Vec<_>>()
            .join("\n")
    ))
}

fn expected_model_hint() -> String {
    env::current_dir()
        .map(|cwd| cwd.join("models").join("nemotron").display().to_string())
        .unwrap_or_else(|_| "models/nemotron".to_string())
}

fn start_audio_capture(
    recording: Arc<AtomicBool>,
    tx: Sender<EngineCommand>,
    events: Sender<EngineEvent>,
    last_voice: Arc<Mutex<Instant>>,
) -> Result<cpal::Stream> {
    let host = cpal::default_host();
    let device = host
        .default_input_device()
        .ok_or_else(|| anyhow!("no default microphone found"))?;
    let input_config = device.default_input_config()?;
    let sample_rate = input_config.sample_rate().0 as f64;
    let channels = input_config.channels() as usize;
    let config = input_config.config();

    let _ = events.send(EngineEvent::status(format!(
        "Ready. Mic: {} Hz, {} channel(s).",
        sample_rate as u32, channels
    )));

    let err_events = events.clone();
    let err_fn = move |err: cpal::StreamError| {
        let _ = err_events.send(EngineEvent::error(format!("Microphone error: {err}")));
    };

    let stream = match input_config.sample_format() {
        cpal::SampleFormat::F32 => {
            let mut callback =
                InputCallback::new(channels, sample_rate, recording, tx, events, last_voice);
            device.build_input_stream(
                &config,
                move |data: &[f32], _| callback.push_f32(data),
                err_fn,
                None,
            )?
        }
        cpal::SampleFormat::I16 => {
            let mut callback =
                InputCallback::new(channels, sample_rate, recording, tx, events, last_voice);
            device.build_input_stream(
                &config,
                move |data: &[i16], _| callback.push_i16(data),
                err_fn,
                None,
            )?
        }
        cpal::SampleFormat::U16 => {
            let mut callback =
                InputCallback::new(channels, sample_rate, recording, tx, events, last_voice);
            device.build_input_stream(
                &config,
                move |data: &[u16], _| callback.push_u16(data),
                err_fn,
                None,
            )?
        }
        other => return Err(anyhow!("unsupported microphone sample format: {other:?}")),
    };

    stream.play()?;
    Ok(stream)
}

struct InputCallback {
    channels: usize,
    tx: Sender<EngineCommand>,
    events: Sender<EngineEvent>,
    recording: Arc<AtomicBool>,
    was_recording: bool,
    resampler: LinearResampler,
    last_voice: Arc<Mutex<Instant>>,
}

impl InputCallback {
    fn new(
        channels: usize,
        source_rate: f64,
        recording: Arc<AtomicBool>,
        tx: Sender<EngineCommand>,
        events: Sender<EngineEvent>,
        last_voice: Arc<Mutex<Instant>>,
    ) -> Self {
        Self {
            channels,
            tx,
            events,
            recording,
            was_recording: false,
            resampler: LinearResampler::new(source_rate, NEMOTRON_SAMPLE_RATE),
            last_voice,
        }
    }

    fn push_f32(&mut self, data: &[f32]) {
        self.process(data.iter().copied());
    }

    fn push_i16(&mut self, data: &[i16]) {
        self.process(data.iter().map(|s| *s as f32 / 32768.0));
    }

    fn push_u16(&mut self, data: &[u16]) {
        self.process(data.iter().map(|s| (*s as f32 / 65535.0) * 2.0 - 1.0));
    }

    fn process<I>(&mut self, samples: I)
    where
        I: Iterator<Item = f32>,
    {
        let now_recording = self.recording.load(Ordering::Relaxed);
        if !now_recording {
            self.was_recording = false;
            return;
        }

        if !self.was_recording {
            self.resampler.reset();
            self.was_recording = true;
        }

        let mut mono = Vec::new();
        let mut frame = Vec::with_capacity(self.channels);
        for sample in samples {
            frame.push(sample);
            if frame.len() == self.channels {
                let sum = frame.iter().sum::<f32>();
                mono.push(sum / self.channels as f32);
                frame.clear();
            }
        }

        if !mono.is_empty() {
            const NOISE_FLOOR: f32 = 0.006;
            const GAIN: f32 = 14.0;
            const SHAPE: f32 = 0.6;
            let rms = (mono.iter().map(|v| v * v).sum::<f32>() / mono.len() as f32).sqrt();
            let norm = ((rms - NOISE_FLOOR) * GAIN).clamp(0.0, 1.0);
            let level = norm.powf(SHAPE);
            let _ = self.events.try_send(EngineEvent::level(level));

            if rms > VOICE_RMS_THRESHOLD {
                if let Ok(mut last_voice) = self.last_voice.lock() {
                    *last_voice = Instant::now();
                }
            } else if self.silence_elapsed() >= Duration::from_secs(SILENCE_AUTO_STOP_SECS)
                && self.recording.swap(false, Ordering::SeqCst)
            {
                let _ = self.tx.try_send(EngineCommand::Stop {
                    fast: false,
                    auto: true,
                });
            }
        }

        let mut out = Vec::new();
        self.resampler.push(&mono, &mut out);
        if !out.is_empty() {
            let _ = self.tx.try_send(EngineCommand::Audio(out));
        }
    }

    fn silence_elapsed(&self) -> Duration {
        self.last_voice
            .lock()
            .map(|last_voice| last_voice.elapsed())
            .unwrap_or_default()
    }
}

struct LinearResampler {
    source_rate: f64,
    target_rate: f64,
    position: f64,
    last_sample: Option<f32>,
}

impl LinearResampler {
    fn new(source_rate: f64, target_rate: f64) -> Self {
        Self {
            source_rate,
            target_rate,
            position: 0.0,
            last_sample: None,
        }
    }

    fn reset(&mut self) {
        self.position = 0.0;
        self.last_sample = None;
    }

    fn push(&mut self, input: &[f32], output: &mut Vec<f32>) {
        if input.is_empty() {
            return;
        }

        let mut data = Vec::with_capacity(input.len() + usize::from(self.last_sample.is_some()));
        if let Some(last) = self.last_sample {
            data.push(last);
        }
        data.extend_from_slice(input);

        let step = self.source_rate / self.target_rate;
        while self.position + 1.0 < data.len() as f64 {
            let index = self.position.floor() as usize;
            let frac = (self.position - index as f64) as f32;
            let a = data[index];
            let b = data[index + 1];
            output.push(a + (b - a) * frac);
            self.position += step;
        }

        self.last_sample = data.last().copied();
        self.position -= (data.len() - 1) as f64;
    }
}

fn home_dir() -> Option<PathBuf> {
    env::var_os("HOME").map(PathBuf::from)
}
