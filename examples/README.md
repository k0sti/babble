# Babble Examples

This directory contains example applications demonstrating various features of the Babble project.

## Audio Recording Examples

### audio_recorder.rs - Full-Featured Audio Recorder with Playback

A complete egui-based GUI application for recording and playing back audio.

**Features:**
- Record audio from your default microphone
- Save recordings as WAV files (44.1kHz stereo, 32-bit float PCM)
- Play, pause, resume, and stop playback
- Real-time recording statistics (duration, sample count)
- Playback progress bar with time display
- Console logging for debugging audio data flow

**Running the example:**
```bash
cargo run --example audio_recorder
```

Note: This example cannot be built with the full babble library due to dependency conflicts. To run it, you'll need to create a minimal Cargo.toml or run it from a separate project.

**How to use:**
1. Click "Start Recording" and speak into your microphone
2. Click "Stop Recording" when done
3. Click "Play" to listen to your recording
4. Use Pause/Resume/Stop to control playback
5. Watch the console for detailed sample statistics

### test_mic_recording.rs - Quick Microphone Test

A simple command-line tool to test your microphone and verify audio recording works.

**Features:**
- Records exactly 3 seconds of audio
- Analyzes the recording for non-silent data
- Shows detailed statistics (sample count, amplitude, etc.)
- Saves to `test_recording.wav`
- Verifies microphone is actually capturing sound

**Running the example:**
```bash
cargo run --example test_mic_recording
```

**Output:**
```
=== Audio Recording Test ===
Using default input device: default
Sample rate: 44100 Hz
Channels: 2

Starting recording for 3 seconds...
PLEASE SPEAK INTO YOUR MICROPHONE NOW!

=== Recording Complete ===
Total samples: 259604
Duration: 2.94 seconds

=== Analyzing Recording ===
Total samples in file: 259604
Non-zero samples: 255440 (98.4%)
Max amplitude: 0.119306
Average amplitude: 0.003984

✓ Recording contains audio data!
```

This tool is perfect for:
- Verifying your microphone is working
- Testing audio input before using the main application
- Debugging audio recording issues
- Quick audio quality checks

## Other Examples

### simple_transcribe.rs

Example demonstrating speech-to-text transcription using Whisper.

**Running:**
```bash
cargo run --example simple_transcribe
```

## Notes

- All audio examples use CPAL (Cross-Platform Audio Library) for audio I/O
- Audio format is standardized to 44.1kHz for compatibility
- Examples are instrumented with logging for debugging
- Requires a working microphone/audio input device
