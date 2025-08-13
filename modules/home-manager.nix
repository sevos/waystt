{ inputs, ... }:
{
  flake.homeManagerModules.hotline =
    {
      config,
      lib,
      pkgs,
      ...
    }:
    let
      cfg = config.programs.hotline;
    in
    {
      options.programs.hotline = {
        enable = lib.mkEnableOption "hotline";

        package = lib.mkOption {
          type = lib.types.package;
          default = inputs.self.packages.${pkgs.system}.hotline;
          defaultText = lib.literalExpression "inputs.self.packages.\${pkgs.system}.hotline";
          description = "The hotline package to use.";
        };

        settings = lib.mkOption {
          type = lib.types.submodule {
            freeformType = lib.types.attrsOf lib.types.str;
            options = {
              OPENAI_API_KEY = lib.mkOption {
                type = lib.types.nullOr lib.types.str;
                default = null;
                description = "OpenAI API key for Whisper transcription.";
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
          description = "Configuration written to {file}`$XDG_CONFIG_HOME/hotline/.env`.";
        };

        systemdService = {
          enable = lib.mkEnableOption "hotline systemd service";
          
          wantedBy = lib.mkOption {
            type = lib.types.listOf lib.types.str;
            default = [ "graphical-session.target" ];
            description = "Systemd targets that want this service.";
          };
        };
      };

      config = lib.mkIf cfg.enable {
        home.packages = [ cfg.package ];

        xdg.configFile."hotline/.env" = lib.mkIf (cfg.settings != { }) {
          text = lib.concatStringsSep "\n" (
            lib.mapAttrsToList (name: value: 
              if value == null then ""
              else if lib.isBool value then "${name}=${if value then "true" else "false"}"
              else "${name}=${toString value}"
            ) cfg.settings
          );
        };

        systemd.user.services.hotline = lib.mkIf cfg.systemdService.enable {
          Unit = {
            Description = "HotLine - Speech-to-text tool";
            After = [ "graphical-session.target" ];
            PartOf = [ "graphical-session.target" ];
          };

          Service = {
            Type = "simple";
            ExecStart = "${cfg.package}/bin/hotline";
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