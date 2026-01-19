# Babble project commands
# Run `just --list` to see all available commands

# Default recipe - show help
default:
    @just --list

# ============================================================================
# Model Downloads
# ============================================================================

# Download all required models
download-models: download-whisper download-vad
    @echo "All models downloaded successfully!"

# Download Whisper model for speech-to-text
download-whisper model="base.en":
    #!/usr/bin/env bash
    set -euo pipefail

    MODEL_DIR="models"
    CACHE_DIR="${HOME}/.cache/whisper"

    # Model URLs from whisper.cpp
    declare -A MODELS=(
        ["tiny.en"]="https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-tiny.en.bin"
        ["tiny"]="https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-tiny.bin"
        ["base.en"]="https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-base.en.bin"
        ["base"]="https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-base.bin"
        ["small.en"]="https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-small.en.bin"
        ["small"]="https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-small.bin"
        ["medium.en"]="https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-medium.en.bin"
        ["medium"]="https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-medium.bin"
        ["large-v3"]="https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-large-v3.bin"
    )

    MODEL="{{model}}"
    URL="${MODELS[$MODEL]:-}"

    if [ -z "$URL" ]; then
        echo "Unknown model: $MODEL"
        echo "Available models: ${!MODELS[*]}"
        exit 1
    fi

    FILENAME="ggml-${MODEL}.bin"

    # Create directories
    mkdir -p "$MODEL_DIR"
    mkdir -p "$CACHE_DIR"

    # Check if already exists
    if [ -f "$MODEL_DIR/$FILENAME" ]; then
        echo "Model already exists: $MODEL_DIR/$FILENAME"
        exit 0
    fi

    if [ -f "$CACHE_DIR/$FILENAME" ]; then
        echo "Found in cache, creating symlink..."
        ln -sf "$CACHE_DIR/$FILENAME" "$MODEL_DIR/$FILENAME"
        echo "Linked: $MODEL_DIR/$FILENAME -> $CACHE_DIR/$FILENAME"
        exit 0
    fi

    echo "Downloading Whisper model: $MODEL"
    echo "URL: $URL"
    echo "Destination: $CACHE_DIR/$FILENAME"

    # Download to cache
    curl -L --progress-bar -o "$CACHE_DIR/$FILENAME" "$URL"

    # Create symlink in models dir
    ln -sf "$CACHE_DIR/$FILENAME" "$MODEL_DIR/$FILENAME"

    echo "Downloaded and linked: $MODEL_DIR/$FILENAME"

# Download Silero VAD model (usually auto-downloaded, but can be pre-fetched)
download-vad:
    #!/usr/bin/env bash
    set -euo pipefail

    CACHE_DIR="${HOME}/.cache/silero-vad"
    MODEL_URL="https://github.com/snakers4/silero-vad/raw/master/files/silero_vad.onnx"

    mkdir -p "$CACHE_DIR"

    if [ -f "$CACHE_DIR/silero_vad.onnx" ]; then
        echo "Silero VAD model already exists: $CACHE_DIR/silero_vad.onnx"
        exit 0
    fi

    echo "Downloading Silero VAD model..."
    curl -L --progress-bar -o "$CACHE_DIR/silero_vad.onnx" "$MODEL_URL"
    echo "Downloaded: $CACHE_DIR/silero_vad.onnx"

# Download VITS TTS model (Piper voices)
download-tts voice="en_US-lessac-medium":
    #!/usr/bin/env bash
    set -euo pipefail

    MODEL_DIR="models/tts"
    CACHE_DIR="${HOME}/.cache/piper"

    VOICE="{{voice}}"

    # Piper voice URLs
    BASE_URL="https://huggingface.co/rhasspy/piper-voices/resolve/main"

    # Parse voice name to get language and variant
    LANG=$(echo "$VOICE" | cut -d'-' -f1-2)
    LANG_PATH=$(echo "$LANG" | tr '_' '/')

    MODEL_URL="${BASE_URL}/${LANG_PATH}/${VOICE}/${VOICE}.onnx"
    CONFIG_URL="${BASE_URL}/${LANG_PATH}/${VOICE}/${VOICE}.onnx.json"

    mkdir -p "$MODEL_DIR"
    mkdir -p "$CACHE_DIR"

    # Check if already exists
    if [ -f "$MODEL_DIR/${VOICE}.onnx" ] && [ -f "$MODEL_DIR/${VOICE}.onnx.json" ]; then
        echo "TTS model already exists: $MODEL_DIR/${VOICE}.onnx"
        exit 0
    fi

    echo "Downloading Piper TTS voice: $VOICE"

    # Download model
    if [ ! -f "$CACHE_DIR/${VOICE}.onnx" ]; then
        echo "Downloading model..."
        curl -L --progress-bar -o "$CACHE_DIR/${VOICE}.onnx" "$MODEL_URL"
    fi

    # Download config
    if [ ! -f "$CACHE_DIR/${VOICE}.onnx.json" ]; then
        echo "Downloading config..."
        curl -L --progress-bar -o "$CACHE_DIR/${VOICE}.onnx.json" "$CONFIG_URL"
    fi

    # Link to models dir
    ln -sf "$CACHE_DIR/${VOICE}.onnx" "$MODEL_DIR/${VOICE}.onnx"
    ln -sf "$CACHE_DIR/${VOICE}.onnx.json" "$MODEL_DIR/${VOICE}.onnx.json"

    echo "Downloaded and linked TTS model: $MODEL_DIR/${VOICE}.onnx"

# List available Whisper models
list-whisper-models:
    @echo "Available Whisper models:"
    @echo "  tiny.en    - 39 MB  (English only, fastest)"
    @echo "  tiny       - 39 MB  (Multilingual)"
    @echo "  base.en    - 74 MB  (English only, recommended)"
    @echo "  base       - 74 MB  (Multilingual)"
    @echo "  small.en   - 244 MB (English only)"
    @echo "  small      - 244 MB (Multilingual)"
    @echo "  medium.en  - 769 MB (English only)"
    @echo "  medium     - 769 MB (Multilingual)"
    @echo "  large-v3   - 1.5 GB (Best quality, multilingual)"
    @echo ""
    @echo "Usage: just download-whisper <model>"
    @echo "Example: just download-whisper base.en"

# ============================================================================
# Build & Run
# ============================================================================

# Build proto app
build:
    cargo build --package proto

# Build proto app in release mode
build-release:
    cargo build --package proto --release

# Run proto app
run *ARGS:
    cargo run --package proto -- {{ARGS}}

# Run proto app with debug logging
run-debug *ARGS:
    RUST_LOG=proto=debug cargo run --package proto -- {{ARGS}}

# Run proto app with trace logging
run-trace *ARGS:
    RUST_LOG=proto=trace cargo run --package proto -- {{ARGS}}

# Run tests
test:
    cargo test --package proto

# Run tests with output
test-verbose:
    cargo test --package proto -- --nocapture

# Check code without building
check:
    cargo check --package proto

# Format code
fmt:
    cargo fmt --package proto

# Run clippy lints
lint:
    cargo clippy --package proto -- -W clippy::all

# ============================================================================
# Development
# ============================================================================

# Setup development environment (download models, check dependencies)
setup: download-models
    @echo "Checking Rust toolchain..."
    rustc --version
    cargo --version
    @echo ""
    @echo "Development environment ready!"
    @echo "Run 'just run' to start the proto app"

# Clean build artifacts
clean:
    cargo clean

# Watch for changes and rebuild
watch:
    cargo watch -x 'check --package proto'
