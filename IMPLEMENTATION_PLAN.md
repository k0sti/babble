# Babble - Comprehensive Implementation Plan

## Project Overview
Babble is a voice-enabled AI assistant application built with Rust, featuring real-time speech-to-text, LLM inference, and text-to-speech capabilities with a desktop GUI.

## Implementation Phases

### Phase 1: Project Foundation & Core Infrastructure

#### 1.1 Project Setup
- **Initialize Rust Project Structure**
  - Create `Cargo.toml` with workspace configuration
  - Set up module structure (`src/main.rs`, `src/lib.rs`)
  - Configure build settings and optimization profiles
  - Set up logging infrastructure (e.g., `env_logger`, `tracing`)

- **Dependency Management**
  - Add all required dependencies to `Cargo.toml`
  - Pin versions for stability
  - Configure feature flags appropriately

#### 1.2 Core Architecture Setup
- **Module Structure**
  ```
  src/
  ├── main.rs              # Application entry point
  ├── lib.rs               # Library root
  ├── app.rs               # Main application state and coordination
  ├── audio/               # Audio processing modules
  │   ├── mod.rs
  │   ├── input.rs         # Audio input handling (cpal)
  │   ├── output.rs        # Audio output handling (cpal)
  │   ├── vad.rs           # Voice activity detection
  │   ├── resampler.rs     # Audio resampling (rubato)
  │   └── buffer.rs        # Ring buffer management
  ├── speech/              # Speech processing
  │   ├── mod.rs
  │   ├── stt.rs           # Speech-to-text (whisper-rs)
  │   └── tts.rs           # Text-to-speech (piper-rs)
  ├── llm/                 # LLM integration
  │   ├── mod.rs
  │   ├── inference.rs     # mistral.rs integration
  │   └── prompts.rs       # Prompt templates and management
  ├── ui/                  # GUI components
  │   ├── mod.rs
  │   ├── app.rs           # Main UI application
  │   ├── messages.rs      # Message list component
  │   ├── controls.rs      # Input controls component
  │   ├── player.rs        # Audio player component
  │   ├── waveform.rs      # Waveform visualization
  │   └── debug.rs         # Debug panel
  ├── messages/            # Message handling
  │   ├── mod.rs
  │   ├── types.rs         # Message types and enums
  │   └── storage.rs       # Message storage/history
  └── utils/               # Utilities
      ├── mod.rs
      └── channels.rs      # Channel management
  ```

- **Communication Architecture**
  - Design channel-based communication between components
  - Define message passing protocols
  - Set up tokio runtime for async operations
  - Use crossbeam-channel for non-audio thread communication

#### 1.3 Data Models
- **Message Types**
  ```rust
  enum MessageContent {
      Text(String),
      Audio(AudioData),
      Image(ImageData),
      File(FileData),
  }

  struct Message {
      id: Uuid,
      sender: Sender, // User or LLM
      content: MessageContent,
      timestamp: DateTime<Utc>,
      metadata: MessageMetadata,
  }
  ```

- **Audio Data Structures**
  ```rust
  struct AudioData {
      samples: Vec<f32>,
      sample_rate: u32,
      channels: u16,
  }

  struct AudioChunk {
      data: Vec<f32>,
      is_speech: bool,
      timestamp: Duration,
  }
  ```

- **Application State**
  ```rust
  struct AppState {
      messages: Vec<Message>,
      recording_state: RecordingState,
      playback_state: PlaybackState,
      llm_state: LLMState,
      debug_info: DebugInfo,
  }
  ```

---

### Phase 2: Audio Pipeline Implementation

#### 2.1 Audio Input (Recording)
- **Setup cpal Audio Input Stream**
  - Enumerate and select input device
  - Configure audio stream parameters (sample rate, channels)
  - Implement error handling for device failures

- **Voice Activity Detection (VAD)**
  - Integrate silero-vad-rs
  - Configure VAD sensitivity parameters
  - Implement chunk-based processing
  - Handle silence detection for segmentation

- **Ring Buffer Management**
  - Implement lock-free ring buffer for audio data
  - Handle overflow/underflow conditions
  - Provide thread-safe read/write interfaces

- **Recording State Management**
  ```rust
  enum RecordingState {
      Idle,
      Recording,
      Paused,
      Processing,
  }
  ```

#### 2.2 Audio Output (Playback)
- **Setup cpal Audio Output Stream**
  - Configure output device
  - Implement playback queue
  - Handle sample rate conversion if needed

- **Audio Player**
  - Implement play/pause/stop functionality
  - Queue management for TTS audio chunks
  - Crossfading between audio segments
  - Volume control

- **Playback State**
  ```rust
  struct PlaybackState {
      current_message: Option<MessageId>,
      position: Duration,
      is_playing: bool,
      queue: VecDeque<AudioData>,
  }
  ```

#### 2.3 Audio Processing Utilities
- **Resampling (rubato)**
  - Implement resampling for different sample rates
  - Handle conversion between Whisper (16kHz) and Piper sample rates
  - Optimize for real-time performance

- **WAV File Handling (hound)**
  - Read/write WAV files for message storage
  - Convert between internal format and WAV
  - Handle different bit depths and channel configurations

---

### Phase 3: Speech Recognition (STT)

#### 3.1 Whisper Integration
- **Model Loading**
  - Download and configure Whisper model (base/small/medium)
  - Implement model caching
  - Handle model initialization errors

- **Real-time Transcription**
  - Process audio chunks as they arrive
  - Implement streaming transcription
  - Handle partial transcription results
  - Implement proper error recovery

- **VAD-based Segmentation**
  - Use VAD to detect speech boundaries
  - Accumulate audio chunks for processing
  - Trigger transcription on silence detection
  - Handle minimum/maximum segment lengths

#### 3.2 Transcription Pipeline
- **Audio Pre-processing**
  - Resample to 16kHz for Whisper
  - Normalize audio levels
  - Apply noise reduction if needed

- **Threading Model**
  - Run transcription on separate thread
  - Use channels to communicate results back to main app
  - Implement backpressure handling

- **Text Output Handling**
  - Accumulate transcribed text
  - Merge with typed text input
  - Update UI in real-time

---

### Phase 4: LLM Integration

#### 4.1 mistral.rs Setup
- **Model Configuration**
  - Select and download Mistral model
  - Configure model parameters (temperature, top_p, etc.)
  - Set up model caching and quantization options

- **Inference Pipeline**
  - Implement streaming inference
  - Handle token generation
  - Process partial responses

#### 4.2 Prompt Engineering
- **System Prompts**
  - Design system prompt for voice assistant behavior
  - Add instructions for marking text segments for TTS
  - Implement context window management

- **Prompt Templates**
  ```rust
  const SYSTEM_PROMPT: &str = "You are Babble, a helpful voice assistant...
  When responding, you can mark parts of your response that should be spoken
  using the format: [SPEAK]text to speak[/SPEAK]...";
  ```

- **TTS Marking Strategy**
  - Parse LLM output for TTS markers
  - Extract segments for immediate TTS conversion
  - Queue segments for voice synthesis

#### 4.3 Streaming Response Handling
- **Token Processing**
  - Accumulate tokens into words/sentences
  - Detect TTS markers in real-time
  - Send marked segments to TTS pipeline immediately

- **Message Management**
  - Create message objects as responses arrive
  - Update UI with partial responses
  - Handle response completion

---

### Phase 5: Text-to-Speech (TTS)

#### 5.1 Piper Integration
- **Model Setup**
  - Download and configure Piper voice model
  - Handle model loading and initialization
  - Configure voice parameters (speed, pitch)

- **Text Processing**
  - Normalize text for TTS (handle abbreviations, numbers)
  - Handle punctuation for natural pauses
  - Process SSML if supported

#### 5.2 Streaming TTS Pipeline
- **Segment-based Synthesis**
  - Convert LLM output segments to speech as they arrive
  - Queue audio chunks for playback
  - Implement smooth transitions between segments

- **Audio Generation Threading**
  - Run TTS on separate thread pool
  - Use channels for audio chunk delivery
  - Handle backpressure and buffering

- **Synchronization**
  - Coordinate TTS generation with playback
  - Ensure smooth continuous playback
  - Handle scenario where text generation is slower than speech

---

### Phase 6: GUI Implementation (egui)

#### 6.1 Application Window Setup
- **eframe Initialization**
  - Create main application window
  - Configure window size and settings
  - Set up egui context and style

- **Layout Structure**
  - Implement main vertical layout
  - Define responsive sizing
  - Handle window resizing

#### 6.2 Message List Component
- **Message Rendering**
  - Display text messages with formatting
  - Show audio message indicators
  - Render image thumbnails
  - Display file attachments
  - Distinguish between user and LLM messages

- **Message List Features**
  - Scrolling with auto-scroll to bottom
  - Message selection
  - Highlight currently playing message
  - Copy message text functionality

#### 6.3 Audio Player Controls
- **Playback Controls UI**
  - Play/pause/stop buttons
  - Next/previous message buttons
  - Volume slider
  - Progress indicator

- **Waveform Visualization**
  - Real-time waveform rendering for current audio
  - Playback position indicator
  - Interactive seeking (optional)

- **State Synchronization**
  - Update UI based on playback state
  - Handle user interactions
  - Reflect audio player state changes

#### 6.4 Input Controls
- **Text Input Field**
  - Multi-line text editor
  - Display transcribed speech alongside typed text
  - Text formatting support

- **Recording Controls**
  - Record/pause button with visual feedback
  - Recording indicator (red dot/animation)
  - Clear button to reset input

- **Send Buttons**
  - Send as text button
  - Send as voice button
  - Disable when appropriate (e.g., empty input)

#### 6.5 Debug Panel
- **State Display**
  - Recording state
  - Transcription status
  - LLM inference state
  - TTS queue status
  - Audio buffer levels
  - Error messages and warnings

- **Performance Metrics**
  - Frame rate
  - Audio latency
  - Processing times
  - Memory usage (optional)

---

### Phase 7: Integration & Data Flow

#### 7.1 Message Flow Architecture
```
User Input (Voice/Text)
    ↓
[Audio Input] → [VAD] → [STT] → [Text Accumulation]
    ↓                              ↓
[Recording UI] ← ← ← ← ← ← ← ← [Combined with Typed Text]
    ↓
[Send Button] → [Create User Message]
    ↓
[LLM Inference] → [Streaming Response]
    ↓
[Parse TTS Markers] → [Extract Segments]
    ↓                         ↓
[Display in UI] ← ← ← [TTS Generation] → [Audio Queue]
                                              ↓
                                         [Audio Player] → [Speaker]
                                              ↓
                                         [Update UI State]
```

#### 7.2 Channel Communication Design
- **Audio Channels**
  - `audio_input_tx/rx`: Raw audio samples
  - `vad_output_tx/rx`: Speech detection events
  - `audio_chunks_tx/rx`: Processed audio chunks
  - `playback_queue_tx/rx`: Audio for playback

- **Processing Channels**
  - `transcription_tx/rx`: Transcribed text
  - `llm_request_tx/rx`: Messages to LLM
  - `llm_response_tx/rx`: LLM responses (streamed)
  - `tts_request_tx/rx`: Text segments for TTS
  - `tts_output_tx/rx`: Generated audio

- **UI Channels**
  - `ui_command_tx/rx`: User actions
  - `ui_update_tx/rx`: State updates for UI

#### 7.3 Error Handling Strategy
- **Error Types**
  ```rust
  enum BabbleError {
      AudioDeviceError(String),
      ModelLoadError(String),
      TranscriptionError(String),
      InferenceError(String),
      TTSError(String),
      IOError(std::io::Error),
  }
  ```

- **Error Recovery**
  - Graceful degradation for non-critical errors
  - User notification via UI
  - Logging for debugging
  - Automatic retry for transient failures

#### 7.4 State Management
- **Centralized State**
  - Single source of truth for app state
  - Thread-safe state access (Arc<Mutex<T>> or channels)
  - State updates trigger UI refreshes

- **State Persistence** (Optional for Phase 7)
  - Save message history to disk
  - Store user preferences
  - Cache models and configurations

---

### Phase 8: Testing & Optimization

#### 8.1 Unit Testing
- **Component Tests**
  - Audio processing functions
  - VAD accuracy
  - Message serialization/deserialization
  - Channel communication
  - State management

#### 8.2 Integration Testing
- **Pipeline Tests**
  - End-to-end audio recording → transcription
  - LLM response → TTS → playback
  - UI interactions → backend processing
  - Error handling and recovery

#### 8.3 Performance Optimization
- **Latency Reduction**
  - Minimize audio buffer sizes while avoiding dropouts
  - Optimize model inference settings
  - Reduce UI update frequency to necessary changes

- **Memory Management**
  - Implement audio buffer recycling
  - Limit message history size
  - Optimize model memory usage

- **CPU Optimization**
  - Profile hot paths
  - Optimize audio processing loops
  - Consider SIMD for audio operations

#### 8.4 User Testing
- **Usability Testing**
  - Test recording workflow
  - Verify playback controls
  - Check responsiveness
  - Validate error messages

---

### Phase 9: Polish & Features

#### 9.1 Enhanced Features
- **Message History**
  - Persist conversations to disk (SQLite or JSON)
  - Load previous conversations
  - Search message history

- **Configuration**
  - User settings UI
  - Model selection
  - Audio device selection
  - Voice selection for TTS
  - LLM parameters tuning

- **File Handling**
  - Drag-and-drop file attachments
  - Image preview
  - File type detection and icons
  - Support for unsupported file type fallbacks

#### 9.2 UI Polish
- **Themes**
  - Light/dark mode
  - Custom color schemes
  - Font customization

- **Animations**
  - Smooth transitions
  - Recording animation
  - Loading indicators
  - Message appearance animations

- **Keyboard Shortcuts**
  - Record/stop recording
  - Send message
  - Play/pause
  - Navigate messages

#### 9.3 Error Handling UI
- **User-Friendly Error Messages**
  - Clear error descriptions
  - Suggested actions
  - Retry mechanisms

- **Toast Notifications**
  - Non-intrusive notifications
  - Auto-dismiss for informational messages
  - Persistent for errors

---

### Phase 10: Deployment & Documentation

#### 10.1 Build & Packaging
- **Release Builds**
  - Configure optimized build profiles
  - Strip debug symbols
  - Test release builds

- **Platform Support**
  - Linux build and testing
  - macOS build (if applicable)
  - Windows build (if applicable)

- **Distribution**
  - Create installation packages
  - Write installation instructions
  - Set up CI/CD for builds

#### 10.2 Documentation
- **User Documentation**
  - Quick start guide
  - Feature overview
  - Troubleshooting guide
  - FAQ

- **Developer Documentation**
  - Architecture overview
  - API documentation (rustdoc)
  - Contributing guidelines
  - Build instructions

- **Model Setup Guide**
  - Where to download models
  - Model installation instructions
  - Model recommendations

---

## Implementation Timeline Estimates

### Critical Path Dependencies
1. **Foundation First**: Audio I/O + VAD → STT → LLM → TTS → GUI
2. **Parallel Work**: GUI can be developed alongside backend components with mock data
3. **Integration Points**: Each major component should have a testable interface

### Suggested Development Order
1. **Week 1-2**: Project setup, audio I/O, basic GUI framework
2. **Week 3-4**: STT integration, VAD implementation
3. **Week 5-6**: LLM integration, prompt engineering
4. **Week 7-8**: TTS integration, audio playback
5. **Week 9-10**: GUI completion, message components
6. **Week 11-12**: Integration, testing, bug fixes
7. **Week 13-14**: Optimization, polish, documentation

---

## Technical Considerations

### Performance Targets
- **Audio Latency**: < 100ms for recording to transcription
- **STT Latency**: < 1s for typical utterances
- **LLM Response Time**: First token < 500ms (model-dependent)
- **TTS Latency**: < 500ms to first audio chunk
- **UI Frame Rate**: 60 FPS

### Resource Requirements
- **RAM**: ~4-8GB (dependent on models)
- **CPU**: Multi-core recommended for parallel processing
- **GPU**: Optional (can accelerate Whisper and mistral.rs)
- **Storage**: ~5-10GB for models

### Model Recommendations
- **Whisper**: `base.en` or `small.en` for balance of speed/accuracy
- **Mistral**: Quantized 7B model for reasonable performance
- **Piper**: High-quality English voice model

---

## Risk Mitigation

### Technical Risks
1. **Model Performance**: Test models early, have fallback options
2. **Audio Synchronization**: Implement robust buffering and timing
3. **Real-time Constraints**: Profile early and often
4. **Cross-platform Audio**: Test cpal on target platforms early

### Scope Risks
1. **Feature Creep**: Stick to core features first, add enhancements later
2. **Model Complexity**: Start with smaller, faster models
3. **UI Complexity**: Build incrementally, test frequently

---

## Success Criteria

### Minimum Viable Product (MVP)
- ✅ Record voice and transcribe to text
- ✅ Send text to LLM and receive response
- ✅ Convert LLM response to speech and play back
- ✅ Display message history
- ✅ Basic UI controls for recording and playback

### Full Feature Set
- ✅ All MVP features
- ✅ Real-time transcription during recording
- ✅ Streaming LLM responses with partial TTS
- ✅ Waveform visualization
- ✅ Debug panel
- ✅ File attachments support
- ✅ Persistent message history
- ✅ Configuration settings

---

## Next Steps

1. **Review this plan** and adjust based on priorities
2. **Set up development environment** with all required tools
3. **Initialize Cargo project** with basic structure
4. **Download and test models** locally
5. **Begin Phase 1 implementation** - project foundation

---

## Appendix: Key Dependencies

```toml
[dependencies]
# LLM
mistral-rs = "0.x"

# Speech-to-Text
whisper-rs = "0.x"
silero-vad-rs = "0.x"

# Text-to-Speech
piper-rs = "0.x"

# Audio
cpal = "0.15"
rubato = "0.15"
hound = "3.5"
ringbuf = "0.3"

# GUI
eframe = "0.28"
egui = "0.28"

# Async & Channels
tokio = { version = "1", features = ["full"] }
crossbeam-channel = "0.5"

# Utilities
uuid = { version = "1", features = ["v4"] }
chrono = "0.4"
serde = { version = "1", features = ["derive"] }
serde_json = "1"
anyhow = "1"
thiserror = "1"
tracing = "0.1"
tracing-subscriber = "0.3"
```

---

*This implementation plan is a living document and should be updated as the project evolves.*
