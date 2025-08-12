{ inputs, ... }:
{
  flake.homeManagerModules.waystt =
    {
      config,
      lib,
      pkgs,
      ...
    }:
    let
      cfg = config.programs.waystt;
    in
    {
      options.programs.waystt = {
        enable = lib.mkEnableOption "waystt";

        package = lib.mkOption {
          type = lib.types.package;
          default = inputs.self.packages.${pkgs.system}.waystt;
          defaultText = lib.literalExpression "inputs.self.packages.\${pkgs.system}.waystt";
          description = "The waystt package to use.";
        };

        settings = lib.mkOption {
          type = lib.types.submodule {
            freeformType = lib.types.attrsOf lib.types.str;
            options = {
              TRANSCRIPTION_PROVIDER = lib.mkOption {
                type = lib.types.enum [ "openai" "google" "google_v2" "google_v2_rest" ];
                default = "openai";
                description = "The transcription provider to use.";
              };

              OPENAI_API_KEY = lib.mkOption {
                type = lib.types.nullOr lib.types.str;
                default = null;
                description = "OpenAI API key for Whisper transcription.";
              };

              GOOGLE_APPLICATION_CREDENTIALS = lib.mkOption {
                type = lib.types.nullOr lib.types.path;
                default = null;
                description = "Path to Google Cloud credentials JSON file.";
              };

              GOOGLE_PROJECT_ID = lib.mkOption {
                type = lib.types.nullOr lib.types.str;
                default = null;
                description = "Google Cloud project ID.";
              };

              GOOGLE_RECOGNIZER_ID = lib.mkOption {
                type = lib.types.nullOr lib.types.str;
                default = null;
                description = "Google Cloud recognizer ID.";
              };

              ENABLE_AUDIO_FEEDBACK = lib.mkOption {
                type = lib.types.bool;
                default = true;
                description = "Enable audio feedback (beeps).";
              };

              BEEP_VOLUME = lib.mkOption {
                type = lib.types.float;
                default = 0.1;
                description = "Volume for audio feedback (0.0 to 1.0).";
              };

              WHISPER_MODEL = lib.mkOption {
                type = lib.types.str;
                default = "whisper-1";
                description = "OpenAI Whisper model to use.";
              };

              WHISPER_LANGUAGE = lib.mkOption {
                type = lib.types.nullOr lib.types.str;
                default = null;
                description = "Language code for transcription (e.g., 'en', 'es').";
              };

              WHISPER_PROMPT = lib.mkOption {
                type = lib.types.nullOr lib.types.str;
                default = null;
                description = "Optional prompt to guide the transcription.";
              };

              WHISPER_TEMPERATURE = lib.mkOption {
                type = lib.types.float;
                default = 0.0;
                description = "Temperature for transcription (0.0 to 1.0).";
              };
            };
          };
          default = { };
          description = "Configuration written to {file}`$XDG_CONFIG_HOME/waystt/.env`.";
        };

        systemdService = {
          enable = lib.mkEnableOption "waystt systemd service";
          
          wantedBy = lib.mkOption {
            type = lib.types.listOf lib.types.str;
            default = [ "graphical-session.target" ];
            description = "Systemd targets that want this service.";
          };
        };
      };

      config = lib.mkIf cfg.enable {
        home.packages = [ cfg.package ];

        xdg.configFile."waystt/.env" = lib.mkIf (cfg.settings != { }) {
          text = lib.concatStringsSep "\n" (
            lib.mapAttrsToList (name: value: 
              if value == null then ""
              else if lib.isBool value then "${name}=${if value then "true" else "false"}"
              else "${name}=${toString value}"
            ) cfg.settings
          );
        };

        systemd.user.services.waystt = lib.mkIf cfg.systemdService.enable {
          Unit = {
            Description = "waystt - Speech-to-text tool for Wayland";
            After = [ "graphical-session.target" ];
            PartOf = [ "graphical-session.target" ];
          };

          Service = {
            Type = "simple";
            ExecStart = "${cfg.package}/bin/waystt";
            Restart = "on-failure";
            RestartSec = "5s";
            
            # Ensure the service can access audio devices
            SupplementaryGroups = [ "audio" ];
          };

          Install = {
            WantedBy = cfg.systemdService.wantedBy;
          };
        };
      };
    };
}