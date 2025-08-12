{ inputs, ... }:
{
  imports = [
    inputs.treefmt-nix.flakeModule
  ];

  perSystem =
    { ... }:
    {
      treefmt = {
        projectRootFile = ".git/config";

        programs = {
          # Rust code formatting
          rustfmt.enable = true;

          # TOML formatting (for Cargo.toml)
          taplo.enable = true;

          # Nix code formatting
          nixfmt.enable = true;
        };

        settings.global.excludes = [
          ".git/*"
          "flake.lock"
          "target/*"
          "relfa/*"  # Exclude the relfa subdirectory
        ];
      };
    };
}