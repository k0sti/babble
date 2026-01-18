use crate::{BabbleError, Result};
use hound::{WavReader, WavWriter, WavSpec, SampleFormat};
use std::path::Path;
use tracing::{debug, info};

/// Write audio samples to a WAV file
///
/// # Arguments
/// * `path` - Path to the output WAV file
/// * `samples` - Audio samples (f32, range -1.0 to 1.0)
/// * `sample_rate` - Sample rate in Hz
/// * `channels` - Number of channels
pub fn write_wav<P: AsRef<Path>>(
    path: P,
    samples: &[f32],
    sample_rate: u32,
    channels: u16,
) -> Result<()> {
    let spec = WavSpec {
        channels,
        sample_rate,
        bits_per_sample: 16,
        sample_format: SampleFormat::Int,
    };

    let mut writer = WavWriter::create(path.as_ref(), spec)
        .map_err(|e| BabbleError::IOError(format!("Failed to create WAV writer: {}", e)))?;

    // Convert f32 samples to i16
    for &sample in samples {
        let sample_i16 = (sample.clamp(-1.0, 1.0) * i16::MAX as f32) as i16;
        writer.write_sample(sample_i16)
            .map_err(|e| BabbleError::IOError(format!("Failed to write sample: {}", e)))?;
    }

    writer.finalize()
        .map_err(|e| BabbleError::IOError(format!("Failed to finalize WAV file: {}", e)))?;

    info!("Wrote {} samples to WAV file: {:?}", samples.len(), path.as_ref());
    Ok(())
}

/// Read audio samples from a WAV file
///
/// # Arguments
/// * `path` - Path to the input WAV file
///
/// # Returns
/// * Tuple of (samples, sample_rate, channels)
pub fn read_wav<P: AsRef<Path>>(path: P) -> Result<(Vec<f32>, u32, u16)> {
    let mut reader = WavReader::open(path.as_ref())
        .map_err(|e| BabbleError::IOError(format!("Failed to open WAV file: {}", e)))?;

    let spec = reader.spec();
    let sample_rate = spec.sample_rate;
    let channels = spec.channels;

    debug!(
        "Reading WAV file: {} Hz, {} channels, {} bits",
        spec.sample_rate, spec.channels, spec.bits_per_sample
    );

    // Read all samples and convert to f32
    let samples: Result<Vec<f32>> = match spec.sample_format {
        SampleFormat::Float => {
            reader
                .samples::<f32>()
                .map(|s| s.map_err(|e| BabbleError::IOError(format!("Failed to read sample: {}", e))))
                .collect()
        }
        SampleFormat::Int => {
            match spec.bits_per_sample {
                16 => {
                    reader
                        .samples::<i16>()
                        .map(|s| {
                            s.map(|sample| sample as f32 / i16::MAX as f32)
                                .map_err(|e| BabbleError::IOError(format!("Failed to read sample: {}", e)))
                        })
                        .collect()
                }
                24 => {
                    reader
                        .samples::<i32>()
                        .map(|s| {
                            s.map(|sample| sample as f32 / 8388608.0) // 2^23
                                .map_err(|e| BabbleError::IOError(format!("Failed to read sample: {}", e)))
                        })
                        .collect()
                }
                32 => {
                    reader
                        .samples::<i32>()
                        .map(|s| {
                            s.map(|sample| sample as f32 / i32::MAX as f32)
                                .map_err(|e| BabbleError::IOError(format!("Failed to read sample: {}", e)))
                        })
                        .collect()
                }
                _ => {
                    return Err(BabbleError::AudioProcessingError(
                        format!("Unsupported bit depth: {}", spec.bits_per_sample)
                    ));
                }
            }
        }
    };

    let samples = samples?;
    info!("Read {} samples from WAV file", samples.len());

    Ok((samples, sample_rate, channels))
}

/// Convert stereo audio to mono by averaging channels
///
/// # Arguments
/// * `samples` - Interleaved stereo samples
///
/// # Returns
/// * Mono audio samples
pub fn stereo_to_mono(samples: &[f32]) -> Vec<f32> {
    samples
        .chunks(2)
        .map(|chunk| {
            if chunk.len() == 2 {
                (chunk[0] + chunk[1]) / 2.0
            } else {
                chunk[0]
            }
        })
        .collect()
}

/// Convert mono audio to stereo by duplicating the channel
///
/// # Arguments
/// * `samples` - Mono audio samples
///
/// # Returns
/// * Interleaved stereo samples
pub fn mono_to_stereo(samples: &[f32]) -> Vec<f32> {
    let mut stereo = Vec::with_capacity(samples.len() * 2);
    for &sample in samples {
        stereo.push(sample);
        stereo.push(sample);
    }
    stereo
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::f32::consts::PI;

    #[test]
    fn test_write_read_wav() {
        let path = "/tmp/test_audio.wav";

        // Generate a 1-second sine wave at 440 Hz
        let sample_rate = 16000;
        let duration = 1.0;
        let frequency = 440.0;
        let samples: Vec<f32> = (0..(sample_rate as f32 * duration) as usize)
            .map(|i| (2.0 * PI * frequency * i as f32 / sample_rate as f32).sin() * 0.5)
            .collect();

        // Write the WAV file
        assert!(write_wav(path, &samples, sample_rate, 1).is_ok());

        // Read it back
        if let Ok((read_samples, read_rate, read_channels)) = read_wav(path) {
            assert_eq!(read_rate, sample_rate);
            assert_eq!(read_channels, 1);
            assert_eq!(read_samples.len(), samples.len());

            // Check that the samples are approximately equal
            // (some precision loss from i16 conversion is expected)
            for (original, read) in samples.iter().zip(read_samples.iter()) {
                assert!((original - read).abs() < 0.001);
            }
        }

        // Clean up
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn test_stereo_to_mono() {
        let stereo = vec![0.5, 0.3, 0.7, 0.1];
        let mono = stereo_to_mono(&stereo);
        assert_eq!(mono.len(), 2);
        assert!((mono[0] - 0.4).abs() < 0.001); // (0.5 + 0.3) / 2
        assert!((mono[1] - 0.4).abs() < 0.001); // (0.7 + 0.1) / 2
    }

    #[test]
    fn test_mono_to_stereo() {
        let mono = vec![0.5, 0.7];
        let stereo = mono_to_stereo(&mono);
        assert_eq!(stereo.len(), 4);
        assert_eq!(stereo, vec![0.5, 0.5, 0.7, 0.7]);
    }
}
