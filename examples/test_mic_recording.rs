// Simple microphone test - records 3 seconds and prints statistics
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{FromSample, Sample};
use hound::WavWriter;
use std::fs::File;
use std::io::BufWriter;
use std::sync::{Arc, Mutex};

type WavWriterHandle = Arc<Mutex<Option<WavWriter<BufWriter<File>>>>>;

fn main() -> Result<(), anyhow::Error> {
    println!("=== Audio Recording Test ===\n");

    let host = cpal::default_host();
    println!("Audio host: {:?}", host.id());

    // List all input devices
    println!("\nAvailable input devices:");
    for (idx, device) in host.input_devices()?.enumerate() {
        println!("  {}. {}", idx, device.name()?);
    }

    let device = host
        .default_input_device()
        .ok_or_else(|| anyhow::anyhow!("No input device available"))?;

    println!("\nUsing default input device: {}", device.name()?);

    let config = device.default_input_config()?;
    println!("Config: {:?}", config);

    let sample_rate = config.sample_rate().0;
    let channels = config.channels();
    let sample_format = config.sample_format();

    println!("  Sample rate: {} Hz", sample_rate);
    println!("  Channels: {}", channels);
    println!("  Sample format: {:?}", sample_format);

    let spec = hound::WavSpec {
        channels: channels as _,
        sample_rate: sample_rate as _,
        bits_per_sample: (sample_format.sample_size() * 8) as _,
        sample_format: if sample_format.is_float() {
            hound::SampleFormat::Float
        } else {
            hound::SampleFormat::Int
        },
    };

    let output_path = "test_recording.wav";
    let writer = WavWriter::create(output_path, spec)?;
    let writer = Arc::new(Mutex::new(Some(writer)));

    let sample_count = Arc::new(Mutex::new(0usize));
    let sample_count_clone = sample_count.clone();

    let writer_clone = writer.clone();
    let err_fn = |err| {
        eprintln!("Stream error: {}", err);
    };

    println!("\nStarting recording for 3 seconds...");
    println!("PLEASE SPEAK INTO YOUR MICROPHONE NOW!\n");

    let stream = match sample_format {
        cpal::SampleFormat::I16 => device.build_input_stream(
            &config.into(),
            move |data: &[i16], _: &_| {
                write_test_data::<i16, i16>(data, &writer_clone, &sample_count_clone)
            },
            err_fn,
            None,
        )?,
        cpal::SampleFormat::F32 => device.build_input_stream(
            &config.into(),
            move |data: &[f32], _: &_| {
                write_test_data::<f32, f32>(data, &writer_clone, &sample_count_clone)
            },
            err_fn,
            None,
        )?,
        format => {
            return Err(anyhow::anyhow!("Unsupported format: {:?}", format));
        }
    };

    stream.play()?;

    // Record for 3 seconds
    std::thread::sleep(std::time::Duration::from_secs(3));

    drop(stream);

    if let Ok(mut guard) = writer.lock() {
        if let Some(writer) = guard.take() {
            writer.finalize()?;
        }
    }

    let total_samples = *sample_count.lock().unwrap();
    let duration = total_samples as f64 / (sample_rate as f64 * channels as f64);

    println!("\n=== Recording Complete ===");
    println!("Total samples: {}", total_samples);
    println!("Duration: {:.2} seconds", duration);
    println!("Output file: {}", output_path);

    // Analyze the recording
    println!("\n=== Analyzing Recording ===");
    let reader = hound::WavReader::open(output_path)?;
    let samples: Vec<f32> = if reader.spec().sample_format == hound::SampleFormat::Float {
        reader.into_samples::<f32>().filter_map(Result::ok).collect()
    } else {
        reader
            .into_samples::<i16>()
            .filter_map(Result::ok)
            .map(|s| s as f32 / i16::MAX as f32)
            .collect()
    };

    if samples.is_empty() {
        println!("ERROR: No samples recorded!");
        return Ok(());
    }

    let non_zero = samples.iter().filter(|&&s| s.abs() > 0.0001).count();
    let max_amplitude = samples.iter().map(|s| s.abs()).fold(0.0f32, f32::max);
    let avg_amplitude = samples.iter().map(|s| s.abs()).sum::<f32>() / samples.len() as f32;

    println!("Total samples in file: {}", samples.len());
    println!("Non-zero samples: {} ({:.1}%)", non_zero, (non_zero as f64 / samples.len() as f64) * 100.0);
    println!("Max amplitude: {:.6}", max_amplitude);
    println!("Average amplitude: {:.6}", avg_amplitude);

    if max_amplitude < 0.001 {
        println!("\n⚠️  WARNING: Recording appears to be silent or very quiet!");
        println!("   Check your microphone settings and volume levels.");
    } else {
        println!("\n✓ Recording contains audio data!");
    }

    Ok(())
}

fn write_test_data<T, U>(input: &[T], writer: &WavWriterHandle, counter: &Arc<Mutex<usize>>)
where
    T: Sample,
    U: Sample + hound::Sample + FromSample<T>,
{
    if let Ok(mut guard) = writer.try_lock() {
        if let Some(writer) = guard.as_mut() {
            for &sample in input.iter() {
                let sample: U = U::from_sample(sample);
                let _ = writer.write_sample(sample);
            }
        }
    }

    if let Ok(mut count) = counter.lock() {
        *count += input.len();
        if *count % 44100 == 0 {
            println!("  Recorded {} samples...", *count);
        }
    }
}
