{ inputs, ... }:
{
  imports = [
    inputs.flake-file.flakeModules.dendritic
  ];

  flake-file = {
    description = "HotLine - A minimalist speech-to-text (STT) tool.";

    inputs = {
      nixpkgs.url = "github:nixos/nixpkgs/nixpkgs-unstable";
      flake-parts.url = "github:hercules-ci/flake-parts";
      systems.url = "github:nix-systems/default";
      devshell.url = "github:numtide/devshell";
      git-hooks-nix.url = "github:cachix/git-hooks.nix";
      treefmt-nix.url = "github:numtide/treefmt-nix";
    };
  };
}