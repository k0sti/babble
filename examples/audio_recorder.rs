use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{FromSample, Sample, Stream};
use eframe::egui;
use hound::WavWriter;
use rodio::{Decoder, OutputStream, Sink};
use std::fs::File;
use std::io::{BufReader, BufWriter};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

fn main() -> eframe::Result<()> {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default().with_inner_size([400.0, 300.0]),
        ..Default::default()
    };
    eframe::run_native(
        "Audio Recorder Example",
        options,
        Box::new(|_cc| Ok(Box::new(AudioRecorderApp::new()))),
    )
}

type WavWriterHandle = Arc<Mutex<Option<WavWriter<BufWriter<File>>>>>;

// Shared state for audio statistics
#[derive(Clone)]
struct AudioStats {
    samples_recorded: Arc<Mutex<usize>>,
    sample_rate: Arc<Mutex<u32>>,
    channels: Arc<Mutex<u16>>,
}

impl AudioStats {
    fn new() -> Self {
        Self {
            samples_recorded: Arc::new(Mutex::new(0)),
            sample_rate: Arc::new(Mutex::new(0)),
            channels: Arc::new(Mutex::new(0)),
        }
    }

    fn get_duration_secs(&self) -> f64 {
        let samples = *self.samples_recorded.lock().unwrap();
        let rate = *self.sample_rate.lock().unwrap();
        let channels = *self.channels.lock().unwrap();
        if rate > 0 && channels > 0 {
            samples as f64 / (rate as f64 * channels as f64)
        } else {
            0.0
        }
    }
}

struct AudioRecorderApp {
    recording: bool,
    stream: Option<Stream>,
    writer: WavWriterHandle,
    output_path: PathBuf,
    status: String,
    device_name: String,
    audio_stats: AudioStats,

    // Playback state
    _output_stream: Option<OutputStream>,
    sink: Option<Sink>,
    playback_start: Option<Instant>,
    playback_duration: Option<Duration>,
    paused_position: Duration,
}

impl AudioRecorderApp {
    fn new() -> Self {
        let output_path = PathBuf::from("recorded.wav");
        Self {
            recording: false,
            stream: None,
            writer: Arc::new(Mutex::new(None)),
            output_path,
            status: "Ready to record".to_string(),
            device_name: String::new(),
            audio_stats: AudioStats::new(),
            _output_stream: None,
            sink: None,
            playback_start: None,
            playback_duration: None,
            paused_position: Duration::from_secs(0),
        }
    }

    fn start_recording(&mut self) {
        if self.recording {
            return;
        }

        match self.try_start_recording() {
            Ok(device_name) => {
                self.recording = true;
                self.device_name = device_name;
                self.status = format!("Recording to: {}", self.output_path.display());
            }
            Err(e) => {
                self.status = format!("Error: {}", e);
            }
        }
    }

    fn try_start_recording(&mut self) -> Result<String, anyhow::Error> {
        let host = cpal::default_host();

        let device = host
            .default_input_device()
            .ok_or_else(|| anyhow::anyhow!("No input device available"))?;

        let device_name = device.name()?;

        let config = device
            .default_input_config()
            .map_err(|e| anyhow::anyhow!("Failed to get default input config: {}", e))?;

        let spec = hound::WavSpec {
            channels: config.channels() as _,
            sample_rate: config.sample_rate().0 as _,
            bits_per_sample: (config.sample_format().sample_size() * 8) as _,
            sample_format: if config.sample_format().is_float() {
                hound::SampleFormat::Float
            } else {
                hound::SampleFormat::Int
            },
        };

        // Reset and set audio stats
        *self.audio_stats.samples_recorded.lock().unwrap() = 0;
        *self.audio_stats.sample_rate.lock().unwrap() = config.sample_rate().0;
        *self.audio_stats.channels.lock().unwrap() = config.channels();

        let writer = WavWriter::create(&self.output_path, spec)?;
        *self.writer.lock().unwrap() = Some(writer);

        let writer_clone = self.writer.clone();
        let stats_clone = self.audio_stats.clone();
        let err_fn = |err| {
            eprintln!("Stream error: {}", err);
        };

        let stream = match config.sample_format() {
            cpal::SampleFormat::I8 => device.build_input_stream(
                &config.into(),
                move |data, _: &_| write_input_data::<i8, i8>(data, &writer_clone, &stats_clone),
                err_fn,
                None,
            )?,
            cpal::SampleFormat::I16 => device.build_input_stream(
                &config.into(),
                move |data, _: &_| write_input_data::<i16, i16>(data, &writer_clone, &stats_clone),
                err_fn,
                None,
            )?,
            cpal::SampleFormat::I32 => device.build_input_stream(
                &config.into(),
                move |data, _: &_| write_input_data::<i32, i32>(data, &writer_clone, &stats_clone),
                err_fn,
                None,
            )?,
            cpal::SampleFormat::F32 => device.build_input_stream(
                &config.into(),
                move |data, _: &_| write_input_data::<f32, f32>(data, &writer_clone, &stats_clone),
                err_fn,
                None,
            )?,
            sample_format => {
                return Err(anyhow::anyhow!(
                    "Unsupported sample format '{}'",
                    sample_format
                ))
            }
        };

        stream.play()?;
        self.stream = Some(stream);

        Ok(device_name)
    }

    fn stop_recording(&mut self) {
        if !self.recording {
            return;
        }

        self.stream = None;
        if let Ok(mut guard) = self.writer.lock() {
            if let Some(writer) = guard.take() {
                match writer.finalize() {
                    Ok(_) => {
                        let duration = self.audio_stats.get_duration_secs();
                        let samples = *self.audio_stats.samples_recorded.lock().unwrap();
                        self.status = format!(
                            "Recording saved: {:.2}s, {} samples",
                            duration, samples
                        );
                    }
                    Err(e) => {
                        self.status = format!("Error saving recording: {}", e);
                    }
                }
            }
        }
        self.recording = false;
    }

    fn start_playback(&mut self) {
        if self.recording {
            self.status = "Cannot play while recording".to_string();
            return;
        }

        if !self.output_path.exists() {
            self.status = "No recording found. Record first!".to_string();
            return;
        }

        match self.try_start_playback() {
            Ok(_) => {
                self.status = format!("Playing: {}", self.output_path.display());
            }
            Err(e) => {
                self.status = format!("Playback error: {}", e);
            }
        }
    }

    fn try_start_playback(&mut self) -> Result<(), anyhow::Error> {
        // Stop any existing playback
        self.stop_playback();

        let (stream, handle) = OutputStream::try_default()?;
        let sink = Sink::try_new(&handle)?;

        let file = File::open(&self.output_path)?;
        let source = Decoder::new(BufReader::new(file))?;

        // Get total duration if possible
        let reader = hound::WavReader::open(&self.output_path)?;
        let spec = reader.spec();
        let duration_secs = reader.duration() as f64 / spec.sample_rate as f64;
        self.playback_duration = Some(Duration::from_secs_f64(duration_secs));

        sink.append(source);
        sink.play();

        self.playback_start = Some(Instant::now() - self.paused_position);
        self._output_stream = Some(stream);
        self.sink = Some(sink);

        Ok(())
    }

    fn pause_playback(&mut self) {
        if let Some(sink) = &self.sink {
            if !sink.is_paused() {
                sink.pause();
                if let Some(start) = self.playback_start {
                    self.paused_position = Instant::now().duration_since(start);
                }
                self.status = "Paused".to_string();
            }
        }
    }

    fn resume_playback(&mut self) {
        if let Some(sink) = &self.sink {
            if sink.is_paused() {
                sink.play();
                self.playback_start = Some(Instant::now() - self.paused_position);
                self.status = "Playing".to_string();
            }
        }
    }

    fn stop_playback(&mut self) {
        self.sink = None;
        self._output_stream = None;
        self.playback_start = None;
        self.paused_position = Duration::from_secs(0);
    }

    fn get_playback_position(&self) -> Duration {
        if let Some(sink) = &self.sink {
            if sink.is_paused() {
                return self.paused_position;
            }
            if let Some(start) = self.playback_start {
                let elapsed = Instant::now().duration_since(start);
                if let Some(duration) = self.playback_duration {
                    return elapsed.min(duration);
                }
                return elapsed;
            }
        }
        Duration::from_secs(0)
    }

    fn is_playing(&self) -> bool {
        if let Some(sink) = &self.sink {
            !sink.is_paused() && !sink.empty()
        } else {
            false
        }
    }

    fn is_paused(&self) -> bool {
        if let Some(sink) = &self.sink {
            sink.is_paused()
        } else {
            false
        }
    }
}

impl eframe::App for AudioRecorderApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // Request continuous repaint for time updates
        ctx.request_repaint();

        egui::CentralPanel::default().show(ctx, |ui| {
            ui.heading("Audio Recorder");
            ui.add_space(20.0);

            ui.horizontal(|ui| {
                ui.label("Output file:");
                ui.text_edit_singleline(&mut self.output_path.to_string_lossy().to_string());
            });

            ui.add_space(10.0);

            if !self.device_name.is_empty() {
                ui.label(format!("Device: {}", self.device_name));
            }

            ui.add_space(20.0);

            // Recording controls
            ui.horizontal(|ui| {
                if ui
                    .add_enabled(!self.recording, egui::Button::new("▶ Start Recording"))
                    .clicked()
                {
                    self.start_recording();
                }

                if ui
                    .add_enabled(self.recording, egui::Button::new("■ Stop Recording"))
                    .clicked()
                {
                    self.stop_recording();
                }
            });

            ui.add_space(10.0);

            if self.recording {
                ui.colored_label(egui::Color32::RED, "● Recording...");
                let duration = self.audio_stats.get_duration_secs();
                let samples = *self.audio_stats.samples_recorded.lock().unwrap();
                ui.label(format!(
                    "Time: {:.2}s | Samples: {}",
                    duration, samples
                ));
            }

            ui.add_space(10.0);
            ui.separator();
            ui.add_space(10.0);

            // Playback controls
            ui.label("Playback:");
            ui.add_space(5.0);

            ui.horizontal(|ui| {
                if ui
                    .add_enabled(!self.recording && !self.is_playing(), egui::Button::new("▶ Play"))
                    .clicked()
                {
                    self.start_playback();
                }

                if ui
                    .add_enabled(self.is_playing(), egui::Button::new("⏸ Pause"))
                    .clicked()
                {
                    self.pause_playback();
                }

                if ui
                    .add_enabled(self.is_paused(), egui::Button::new("▶ Resume"))
                    .clicked()
                {
                    self.resume_playback();
                }

                if ui
                    .add_enabled(self.is_playing() || self.is_paused(), egui::Button::new("■ Stop"))
                    .clicked()
                {
                    self.stop_playback();
                    self.status = "Playback stopped".to_string();
                }
            });

            ui.add_space(10.0);

            // Display playback time
            if self.is_playing() || self.is_paused() {
                let current = self.get_playback_position();
                let total = self.playback_duration.unwrap_or(Duration::from_secs(0));

                let current_secs = current.as_secs_f64();
                let total_secs = total.as_secs_f64();

                ui.label(format!(
                    "Time: {:.2}s / {:.2}s",
                    current_secs, total_secs
                ));

                // Progress bar
                let progress = if total_secs > 0.0 {
                    (current_secs / total_secs) as f32
                } else {
                    0.0
                };
                ui.add(egui::ProgressBar::new(progress).show_percentage());
            }

            ui.add_space(10.0);
            ui.separator();
            ui.add_space(10.0);

            ui.label(&self.status);
        });
    }
}

fn write_input_data<T, U>(input: &[T], writer: &WavWriterHandle, stats: &AudioStats)
where
    T: Sample,
    U: Sample + hound::Sample + FromSample<T>,
{
    if let Ok(mut guard) = writer.try_lock() {
        if let Some(writer) = guard.as_mut() {
            let mut count = 0;
            for &sample in input.iter() {
                let sample: U = U::from_sample(sample);
                let _ = writer.write_sample(sample);
                count += 1;
            }

            // Update statistics
            if let Ok(mut samples_recorded) = stats.samples_recorded.lock() {
                *samples_recorded += count;
            }

            // Log every ~1 second of audio data to verify recording
            if let Ok(samples) = stats.samples_recorded.lock() {
                let rate = *stats.sample_rate.lock().unwrap();
                let channels = *stats.channels.lock().unwrap();
                if rate > 0 && channels > 0 {
                    let duration_secs = *samples as f64 / (rate as f64 * channels as f64);
                    // Log at 1-second intervals
                    if duration_secs.floor() as u32 % 1 == 0
                        && (*samples % (rate * channels as u32) as usize) < count
                    {
                        println!(
                            "Recording: {:.0}s | {} samples | buffer: {} samples",
                            duration_secs, *samples, count
                        );
                    }
                }
            }
        }
    }
}
