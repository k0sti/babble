# Babble

Text-to-speech CLI using [Chatterbox TTS](https://github.com/resemble-ai/chatterbox).

## Setup

Enter the development shell:

```bash
nix develop
```

### Python Server

Create a Python environment and install dependencies:

```bash
uv venv && source .venv/bin/activate
uv pip install chatterbox-tts torchaudio
```

Start the TTS server:

```bash
python server.py
```

Options:
- `--port PORT` - Server port (default: 8787)
- `--model turbo|multilingual|original` - Model variant (default: turbo)
- `--device cuda|cpu` - Device to run on (default: cuda)

### Rust Client

Build the client:

```bash
cargo build --release
```

## Usage

Speak text directly:

```bash
./target/release/babble -t "Hello, world!"
```

Read from stdin:

```bash
echo "Hello from stdin" | ./target/release/babble
```

Interactive mode (speak each line):

```bash
./target/release/babble --interactive
```

Save to file instead of playing:

```bash
./target/release/babble -t "Save this" -o output.wav
```

Voice cloning with reference audio:

```bash
./target/release/babble -t "Clone my voice" -a reference.wav
```

## Options

```
-t, --text <TEXT>           Text to synthesize
-s, --server <URL>          Server URL (default: http://127.0.0.1:8787)
-a, --audio-prompt <PATH>   Reference audio for voice cloning
-e, --exaggeration <FLOAT>  Exaggeration factor 0.0-1.0 (default: 0.5)
-c, --cfg-weight <FLOAT>    CFG weight 0.0-1.0 (default: 0.5)
-o, --output <PATH>         Save audio to file instead of playing
    --interactive           Read input line by line from stdin
```

## MCP Server

Babble includes an MCP server for integration with Claude Code and other MCP clients.

### Claude Code Configuration

Add to your Claude Code MCP settings (`~/.claude/claude_desktop_config.json`):

```json
{
  "mcpServers": {
    "babble": {
      "command": "/path/to/babble-mcp",
      "env": {
        "BABBLE_SERVER": "http://127.0.0.1:8787"
      }
    }
  }
}
```

### Available Tools

- `say` - Convert text to speech and play it aloud
  - `text` (string, required): The text to speak
