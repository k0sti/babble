# Default recipe
default:
    @just --list

# Setup Python environment and install dependencies
setup:
    #!/usr/bin/env bash
    if [ ! -d .venv ]; then
        uv venv
    fi
    source .venv/bin/activate
    uv pip install chatterbox-tts torchaudio

# Start the TTS server
serve model="turbo": setup
    #!/usr/bin/env bash
    source .venv/bin/activate
    python server.py --model {{model}}

# Build the Rust client
build:
    cargo build --release

# Run TTS on text
say text: build
    ./target/release/babble -t "{{text}}"
