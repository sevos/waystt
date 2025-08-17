{ pkgs ? import <nixpkgs> {} }:

pkgs.mkShell {
  buildInputs = with pkgs; [
    # Rust toolchain
    rustc
    cargo
    rustfmt
    rust-analyzer
    clippy
    
    # Build dependencies for audio libraries
    pkg-config
    alsa-lib
    
    # SSL/TLS dependencies
    openssl
    openssl.dev
    
    # Clang dependencies for whisper-rs bindgen
    clang
    libclang.lib
    
    # Build tools for whisper-rs
    cmake
    
    # Development tools
    git
  ];

  # Environment variables for development
  RUST_SRC_PATH = "${pkgs.rust.packages.stable.rustPlatform.rustLibSrc}";
  LIBCLANG_PATH = "${pkgs.libclang.lib}/lib";
  
  shellHook = ''
    echo "Rust development environment loaded"
    echo "Rust version: $(rustc --version)"
    echo "Cargo version: $(cargo --version)"
    echo "LIBCLANG_PATH: $LIBCLANG_PATH"
  '';
}