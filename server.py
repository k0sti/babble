#!/usr/bin/env python3
"""
Chatterbox TTS HTTP server.
Run with: python server.py [--port PORT] [--model turbo|multilingual|original]
"""

# Patch perth to use DummyWatermarker (the real one is proprietary)
import perth
perth.PerthImplicitWatermarker = perth.DummyWatermarker

import argparse
import io
import json
from http.server import HTTPServer, BaseHTTPRequestHandler

import torchaudio


class TTSHandler(BaseHTTPRequestHandler):
    model = None

    def do_POST(self):
        if self.path == "/synthesize":
            content_length = int(self.headers.get("Content-Length", 0))
            body = self.rfile.read(content_length)

            try:
                data = json.loads(body)
                text = data.get("text", "")
                audio_prompt = data.get("audio_prompt")  # Optional path to reference audio
                exaggeration = data.get("exaggeration", 0.5)
                cfg_weight = data.get("cfg_weight", 0.5)

                if not text:
                    self._send_error(400, "Missing 'text' field")
                    return

                # Generate speech
                kwargs = {"text": text}
                if audio_prompt:
                    kwargs["audio_prompt_path"] = audio_prompt
                if hasattr(TTSHandler.model, "generate"):
                    # Turbo model
                    wav = TTSHandler.model.generate(**kwargs)
                else:
                    # Original model has exaggeration/cfg_weight
                    kwargs["exaggeration"] = exaggeration
                    kwargs["cfg_weight"] = cfg_weight
                    wav = TTSHandler.model.generate(**kwargs)

                # Convert to WAV bytes and send directly
                buffer = io.BytesIO()
                torchaudio.save(buffer, wav, TTSHandler.model.sr, format="wav")
                audio_bytes = buffer.getvalue()

                self.send_response(200)
                self.send_header("Content-Type", "audio/wav")
                self.send_header("Content-Length", len(audio_bytes))
                self.end_headers()
                self.wfile.write(audio_bytes)

            except Exception as e:
                self._send_error(500, str(e))
        else:
            self._send_error(404, "Not found")

    def do_GET(self):
        if self.path == "/health":
            self._send_json(200, {"status": "ok", "model_loaded": TTSHandler.model is not None})
        else:
            self._send_error(404, "Not found")

    def _send_json(self, status, data):
        self.send_response(status)
        self.send_header("Content-Type", "application/json")
        self.end_headers()
        self.wfile.write(json.dumps(data).encode())

    def _send_error(self, status, message):
        self._send_json(status, {"error": message})

    def log_message(self, format, *args):
        print(f"[TTS Server] {args[0]}")


def load_model(model_type: str, device: str):
    """Load the specified Chatterbox model."""
    print(f"Loading Chatterbox {model_type} model on {device}...")

    if model_type == "turbo":
        from chatterbox.tts_turbo import ChatterboxTurboTTS
        model = ChatterboxTurboTTS.from_pretrained(device=device)
    elif model_type == "multilingual":
        from chatterbox.tts_multilingual import ChatterboxMultilingualTTS
        model = ChatterboxMultilingualTTS.from_pretrained(device=device)
    else:  # original
        from chatterbox.tts import ChatterboxTTS
        model = ChatterboxTTS.from_pretrained(device=device)

    print(f"Model loaded successfully!")
    return model


def main():
    parser = argparse.ArgumentParser(description="Chatterbox TTS HTTP Server")
    parser.add_argument("--port", type=int, default=8787, help="Port to listen on")
    parser.add_argument("--host", default="127.0.0.1", help="Host to bind to")
    parser.add_argument("--model", choices=["turbo", "multilingual", "original"],
                        default="turbo", help="Model variant to use")
    parser.add_argument("--device", default="cuda", help="Device to run on (cuda/cpu)")
    args = parser.parse_args()

    TTSHandler.model = load_model(args.model, args.device)

    server = HTTPServer((args.host, args.port), TTSHandler)
    print(f"TTS Server running at http://{args.host}:{args.port}")
    print("Endpoints:")
    print("  POST /synthesize - Generate speech from text")
    print("  GET /health - Health check")

    try:
        server.serve_forever()
    except KeyboardInterrupt:
        print("\nShutting down...")
        server.shutdown()


if __name__ == "__main__":
    main()
