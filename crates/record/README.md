# Audio Recorder Example

A minimal egui application demonstrating audio recording using CPAL (Cross-Platform Audio Library).

## Features

- Simple egui UI with Start/Stop recording buttons
- Records audio from the default input device (microphone)
- Saves recordings as WAV files
- Real-time recording status indicator
- Error handling and status messages

## Running the Example

```bash
cargo run -p audio-recorder-example
```

Or from the workspace root:

```bash
cargo run --bin audio-recorder-example
```

## How It Works

This example uses:
- **eframe/egui**: For the graphical user interface
- **cpal**: For cross-platform audio input/output
- **hound**: For WAV file writing

The recording flow:
1. Click "Start Recording" to begin capturing audio
2. Audio is captured in real-time from your default microphone
3. Samples are written to a WAV file as they arrive
4. Click "Stop Recording" to finalize and save the file
5. The recorded audio is saved as `recorded.wav` in the current directory

## Testing

1. Run the application
2. Click "Start Recording"
3. Speak into your microphone for a few seconds
4. Click "Stop Recording"
5. Play back `recorded.wav` to verify the recording

You can play back the recorded file using any audio player:
```bash
# Linux
aplay recorded.wav

# Or use any media player
mpv recorded.wav
```

## Notes

- Make sure your system has a working microphone/input device
- The application will use the system's default input device
- Recording format: 16-bit PCM WAV (or device default format)
- Output file is overwritten on each recording session
