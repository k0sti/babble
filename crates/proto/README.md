
- record user speech with microphone
- audio is sent to concurrent speech-to-text processor
- speech processor detects first word from speech
- if recording starts with 'stop', stop LLM run
- when audio recording stops, inject text into llm run and continue
- LLM starts when any input is received in injected into llm context


### concurrent processes
- LLM run with mistral.rs
- stt-processor listens incoming audio record buffers and translates it streaming into message handler
- message handler checks start of message for code words and instructs llm
- User's audio recording

## Tech
Use crates defined in app crate.

## UI
- streamed LLM text context
- record button to toggle recording on/off
- animated waveform of recorded speech from short interval

- color state indicators for each concurrent prosessor showing:
  - orange: waiting
  - green: processing/running
  - red: error
