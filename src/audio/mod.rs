pub mod buffer;
#[cfg(feature = "audio-io")]
pub mod input;
#[cfg(feature = "audio-io")]
pub mod output;
pub mod resampler;
pub mod vad;
pub mod wav;

pub use buffer::AudioRingBuffer;
#[cfg(feature = "audio-io")]
pub use input::AudioInput;
#[cfg(feature = "audio-io")]
pub use output::AudioOutput;
pub use resampler::AudioResampler;
pub use vad::VoiceActivityDetector;
pub use wav::{write_wav, read_wav};

use crate::Result;
use tracing::info;

/// Test function to verify audio pipeline functionality
pub fn test_audio_pipeline() -> Result<()> {
    info!("Testing audio pipeline...");

    // Test 1: Ring buffer
    info!("Testing ring buffer...");
    let mut buffer = AudioRingBuffer::new(1024);
    let test_data: Vec<f32> = (0..512).map(|i| i as f32 / 512.0).collect();
    buffer.write(&test_data);
    let read_data = buffer.read(512);
    assert_eq!(read_data.len(), 512);
    info!("✓ Ring buffer test passed!");

    // Test 2: WAV file handling
    info!("Testing WAV file handling...");
    let wav_path = "/tmp/babble_test.wav";
    let test_samples: Vec<f32> = (0..16000).map(|i| {
        (i as f32 * 440.0 * 2.0 * std::f32::consts::PI / 16000.0).sin() * 0.5
    }).collect();
    write_wav(wav_path, &test_samples, 16000, 1)?;
    let (read_samples, sample_rate, channels) = read_wav(wav_path)?;
    assert_eq!(sample_rate, 16000);
    assert_eq!(channels, 1);
    assert_eq!(read_samples.len(), test_samples.len());
    std::fs::remove_file(wav_path).ok();
    info!("✓ WAV file handling test passed!");

    // Test 3: Resampler
    info!("Testing audio resampler...");
    let mut resampler = AudioResampler::new(16000, 48000, 1)?;
    let input: Vec<f32> = (0..1600).map(|i| (i as f32 * 0.01).sin()).collect();
    let output = resampler.resample(&input)?;
    assert!(!output.is_empty());
    info!("✓ Resampler test passed!");

    // Test 4: VAD
    info!("Testing Voice Activity Detection...");
    if let Ok(mut vad) = VoiceActivityDetector::new(16000, 0.5) {
        let silence = vec![0.0f32; 512];
        let is_speech = vad.is_speech(&silence)?;
        assert!(!is_speech);
        info!("✓ VAD test passed!");
    } else {
        info!("⚠ VAD test skipped (model not available)");
    }

    info!("✅ All audio pipeline tests passed!");
    Ok(())
}
