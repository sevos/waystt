{ inputs, ... }:
{
  imports = [
    inputs.git-hooks-nix.flakeModule
  ];

  perSystem =
    { pkgs, ... }:
    {
      pre-commit = {
        check.enable = true;

        settings = {
          hooks = {
            # Rust formatting check (matches CI: cargo fmt --all -- --check)
            rustfmt = {
              enable = true;
              packageOverrides.cargo = pkgs.cargo;
              packageOverrides.rustfmt = pkgs.rustfmt;
            };

            # Rust linting (matches CI: cargo clippy --all-targets --all-features -- -D warnings)
            cargo-clippy = {
              enable = true;
              name = "cargo-clippy";
              entry = "${pkgs.cargo}/bin/cargo clippy --all-features -- -D warnings";
              language = "system";
              files = "\\.rs$";
              pass_filenames = false;
            };

            # Run cargo test with BEEP_VOLUME=0.0 (matches CI: cargo test --verbose)
            cargo-test = {
              enable = true;
              name = "cargo-test";
              entry = let
                cargo_test = pkgs.writeShellScriptBin "cargo-test" ''
                  BEEP_VOLUME=0.0 RUSTFLAGS="-A dead_code" ${pkgs.cargo}/bin/cargo test --verbose "$@"
                '';
              in "${cargo_test}/bin/cargo-test";
              language = "system";
              files = "\\.(rs|toml)$";
              pass_filenames = false;
            };

            # Update Cargo.lock when Cargo.toml is modified
            cargo-lock = {
              enable = true;
              name = "cargo-lock";
              entry = "${pkgs.cargo}/bin/cargo generate-lockfile";
              language = "system";
              files = "Cargo\\.toml$";
              pass_filenames = false;
            };
          };
        };
      };

      # Pre-commit hooks are automatically installed when entering the devshell
    };
}
