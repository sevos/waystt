{ ... }:
{
  perSystem =
    { config, pkgs, ... }:
    let
      cargoToml = builtins.fromTOML (builtins.readFile ../Cargo.toml);
    in
    {
      packages.waystt = pkgs.rustPlatform.buildRustPackage {
        pname = "waystt";
        version = cargoToml.package.version;

        src = ../.;

        cargoLock = {
          lockFile = ../Cargo.lock;
        };

        nativeBuildInputs = with pkgs; [
          pkg-config
          clang
        ];

        buildInputs =
          with pkgs;
          [
            # Audio dependencies
            alsa-lib
            pipewire
            
            # SSL/TLS for reqwest
            openssl
            
            # System libraries
            dbus
          ]
          ++ pkgs.lib.optionals pkgs.stdenv.isDarwin [
            pkgs.darwin.apple_sdk.frameworks.AudioUnit
            pkgs.darwin.apple_sdk.frameworks.CoreAudio
            pkgs.darwin.apple_sdk.frameworks.CoreFoundation
            pkgs.darwin.apple_sdk.frameworks.Security
          ];

        # Set environment variables for the build
        LIBCLANG_PATH = "${pkgs.llvmPackages.libclang.lib}/lib";
        
        # Ensure tests run with audio feedback disabled
        checkPhase = ''
          runHook preCheck
          export BEEP_VOLUME=0.0
          cargo test --release
          runHook postCheck
        '';

        meta = with pkgs.lib; {
          description = cargoToml.package.description;
          homepage = "https://github.com/nilp0inter/waystt";
          license = licenses.gpl3Plus;
          maintainers = with maintainers; [ ];
          mainProgram = "waystt";
          platforms = platforms.linux ++ platforms.darwin;
        };
      };

      # Make waystt the default package
      packages.default = config.packages.waystt;
    };
}
