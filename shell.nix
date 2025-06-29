{ pkgs ? import <nixpkgs> {} }:

pkgs.mkShell {
  buildInputs = with pkgs; [
    # System dependencies from CI workflows and Cargo.toml
    alsa-lib
    libpulseaudio
    jack2
    pipewire
    openssl
    pkg-config
  ];

  nativeBuildInputs = with pkgs; [
    # Rust toolchain
    rustc
    cargo
    clippy
    rustfmt

    # Additional cargo tools from CI
    cargo-machete
    cargo-audit
  ];

  # Set environment variables for cargo to find openssl
  OPENSSL_DIR = "${pkgs.openssl.dev}";
  OPENSSL_LIB_DIR = "${pkgs.openssl.out}/lib";
  OPENSSL_INCLUDE_DIR = "${pkgs.openssl.dev}/include";
}
