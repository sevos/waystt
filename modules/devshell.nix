{ inputs, ... }:
{
  imports = [
    inputs.devshell.flakeModule
  ];

  perSystem =
    { config, pkgs, ... }:
    {
      devshells.default = {
        packages =
          with pkgs;
          [
            # Rust toolchain
            cargo
            rustc
            rust-analyzer
            clippy
            rustfmt
            clang
            
            # Build dependencies
            pkg-config
            
            # Audio dependencies
            alsa-lib
            pipewire
            
            # SSL/TLS for reqwest
            openssl
            
            # For Google Cloud auth
            cacert
          ]
          ++ [
            config.treefmt.build.wrapper
          ];

        commands = [
          {
            help = "Run cargo build";
            name = "build";
            command = "cargo build";
          }
          {
            help = "Run cargo test";
            name = "test";
            command = "BEEP_VOLUME=0.0 cargo test";
          }
          {
            help = "Run cargo check";
            name = "check";
            command = "cargo check";
          }
          {
            help = "Format all files";
            name = "fmt";
            command = "treefmt";
          }
          {
            help = "Install git hooks";
            name = "install-hooks";
            command = "pre-commit install";
          }
          {
            help = "Run HotLine with local .env file";
            name = "run-local";
            command = "cargo run -- --envfile .env";
          }
          {
            help = "Run HotLine in release mode with local .env";
            name = "run-release";
            command = "cargo run --release -- --envfile .env";
          }
        ];

        env = [
          {
            name = "RUST_SRC_PATH";
            value = "${pkgs.rust.packages.stable.rustPlatform.rustLibSrc}";
          }
          {
            name = "PKG_CONFIG_PATH";
            value = "${pkgs.alsa-lib.dev}/lib/pkgconfig:${pkgs.pipewire.dev}/lib/pkgconfig:${pkgs.openssl.dev}/lib/pkgconfig";
          }
        ];

        devshell.startup.pre-commit-install = {
          text = ''
            ${config.pre-commit.installationScript}
            git config --local --unset core.hooksPath || true
            pre-commit install --install-hooks
          '';
        };
      };
    };
}
