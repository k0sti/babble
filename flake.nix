{
  description = "Babble - A voice-enabled AI assistant with real-time speech processing";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    flake-utils.url = "github:numtide/flake-utils";
    rust-overlay = {
      url = "github:oxalica/rust-overlay";
      inputs.nixpkgs.follows = "nixpkgs";
    };
  };

  outputs = { self, nixpkgs, flake-utils, rust-overlay }:
    flake-utils.lib.eachDefaultSystem (system:
      let
        overlays = [ (import rust-overlay) ];
        pkgs = import nixpkgs {
          inherit system overlays;
        };

        # Get the Rust toolchain from rust-toolchain.toml or use stable
        rustToolchain = pkgs.rust-bin.stable.latest.default.override {
          extensions = [ "rust-src" "rust-analyzer" ];
        };

        # System-specific dependencies for audio libraries
        buildInputs = with pkgs; [
          # Audio libraries for cpal
          alsa-lib
          alsa-lib.dev

          # TTS dependencies for piper-rs/sherpa-rs
          espeak-ng
          sonic

          # Additional dependencies that might be needed
          openssl

          # GUI dependencies for eframe/egui
          libxkbcommon
          libGL

          # X11 backend
          xorg.libX11
          xorg.libXcursor
          xorg.libXrandr
          xorg.libXi

          # Wayland backend
          wayland
        ] ++ lib.optionals stdenv.isDarwin [
          # macOS-specific audio frameworks
          darwin.apple_sdk.frameworks.CoreAudio
          darwin.apple_sdk.frameworks.AudioToolbox
        ];

        nativeBuildInputs = with pkgs; [
          pkg-config
          # Required for bindgen (whisper-rs-sys)
          clang
          llvmPackages.libclang
          cmake
        ];

      in
      {
        devShells.default = pkgs.mkShell {
          inherit buildInputs nativeBuildInputs;

          packages = with pkgs; [
            rustToolchain
            cargo
            rustc
            clippy
            rustfmt
          ];

          # Environment variables
          RUST_SRC_PATH = "${rustToolchain}/lib/rustlib/src/rust/library";

          # For bindgen to find libclang
          LIBCLANG_PATH = "${pkgs.llvmPackages.libclang.lib}/lib";

          # For ALSA on Linux
          LD_LIBRARY_PATH = pkgs.lib.makeLibraryPath buildInputs;

          shellHook = ''
            echo "🦀 Rust development environment for Babble"
            echo "Rust version: $(rustc --version)"
            echo "Cargo version: $(cargo --version)"
            echo ""
            echo "Ready to build! Try: cargo build"
          '';
        };

        # Optional: Define a package for building Babble
        packages.default = pkgs.rustPlatform.buildRustPackage {
          pname = "babble";
          version = "0.1.0";

          src = ./.;

          cargoLock = {
            lockFile = ./Cargo.lock;
          };

          inherit buildInputs nativeBuildInputs;

          meta = with pkgs.lib; {
            description = "A voice-enabled AI assistant with real-time speech processing";
            license = licenses.mit;
          };
        };
      }
    );
}
