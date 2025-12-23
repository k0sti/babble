{
  description = "Babble - Text-to-speech CLI using Chatterbox TTS";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    flake-utils.url = "github:numtide/flake-utils";
  };

  outputs = { self, nixpkgs, flake-utils }:
    flake-utils.lib.eachDefaultSystem (system:
      let
        pkgs = import nixpkgs {
          inherit system;
          config.allowUnfree = true;
          config.cudaSupport = true;
        };
      in
      {
        devShells.default = pkgs.mkShell {
          buildInputs = with pkgs; [
            # Rust
            rustc
            cargo
            rust-analyzer
            clippy
            rustfmt

            # Audio libraries for rodio
            alsa-lib
            pkg-config

            # Python with uv (3.11 needed for chatterbox-tts numpy compatibility)
            uv
            python311

            # For PyTorch
            stdenv.cc.cc.lib
            zlib

            # CUDA
            cudaPackages.cuda_cudart
            cudaPackages.cuda_nvcc
          ];

          shellHook = ''
            export PKG_CONFIG_PATH="${pkgs.alsa-lib.dev}/lib/pkgconfig:$PKG_CONFIG_PATH"
            export LD_LIBRARY_PATH="${pkgs.lib.makeLibraryPath [
              pkgs.alsa-lib
              pkgs.stdenv.cc.cc.lib
              pkgs.zlib
              pkgs.cudaPackages.cuda_cudart
              pkgs.cudaPackages.cuda_nvcc
            ]}:/run/opengl-driver/lib:$LD_LIBRARY_PATH"
            export CUDA_PATH="${pkgs.cudaPackages.cuda_nvcc}"
            echo "Babble development environment"
            echo ""
            echo "Setup Python environment:"
            echo "  uv venv && source .venv/bin/activate"
            echo "  uv pip install chatterbox-tts torchaudio"
            echo ""
            echo "Run TTS server:"
            echo "  python server.py"
            echo ""
            echo "Build and run Rust client:"
            echo "  cargo build --release"
            echo "  ./target/release/babble -t 'Hello world'"
          '';
        };

        packages.default = pkgs.rustPlatform.buildRustPackage {
          pname = "babble";
          version = "0.1.0";
          src = ./.;
          cargoLock.lockFile = ./Cargo.lock;

          nativeBuildInputs = [ pkgs.pkg-config ];
          buildInputs = [ pkgs.alsa-lib ];
        };
      });
}
