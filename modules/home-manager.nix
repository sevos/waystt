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
      inherit (lib)
        literalExpression
        mkEnableOption
        mkIf
        mkOption
        types
        ;

      finalSettings = cfg.settings;
    in
    {
      options.programs.hotline = {
        enable = mkEnableOption "Hotline";

        package = mkOption {
          type = types.package;
          default = inputs.self.packages.${pkgs.system}.hotline;
          defaultText = literalExpression "inputs.self.packages.${pkgs.system}.hotline";
          description = "The package to use for Hotline.";
        };

        settings =
          let
            hookType = types.submodule {
              options = {
                type = mkOption {
                  type = types.enum [
                    "spawn"
                    "spawn_with_stdin"
                  ];
                  description = "The type of command to execute.";
                };
                command = mkOption {
                  type = types.listOf types.str;
                  description = "The command to execute.";
                };
              };
            };

            vadConfigType = types.submodule {
              options = {
                ServerVad = mkOption {
                  type = types.nullOr (types.submodule {
                    options = {
                      threshold = mkOption {
                        type = types.nullOr types.float;
                        default = null;
                      };
                      prefix_padding_ms = mkOption {
                        type = types.nullOr types.ints.unsigned;
                        default = null;
                      };
                      silence_duration_ms = mkOption {
                        type = types.nullOr types.ints.unsigned;
                        default = null;
                      };
                    };
                  });
                  default = null;
                  description = "Configuration for server-side Voice Activity Detection (VAD).";
                };
                SemanticVad = mkOption {
                  type = types.nullOr (types.submodule {
                    options = {
                      eagerness = mkOption {
                        type = types.nullOr (types.enum [
                          "low"
                          "medium"
                          "high"
                          "very-high"
                        ]);
                        default = null;
                      };
                    };
                  });
                  default = null;
                  description = "Configuration for semantic Voice Activity Detection (VAD).";
                };
              };
            };

            profileType = types.submodule {
              options = {
                model = mkOption {
                  type = types.nullOr types.str;
                  default = null;
                  description = "The model to use for this profile.";
                };
                language = mkOption {
                  type = types.nullOr types.str;
                  default = null;
                  description = "The language to use for this profile.";
                };
                prompt = mkOption {
                  type = types.nullOr types.str;
                  default = null;
                  description = "The prompt to use for this profile.";
                };
                hooks = mkOption {
                  type = types.nullOr (types.submodule {
                    options = {
                      on_transcription_start = mkOption {
                        type = types.nullOr hookType;
                        default = null;
                        description = "Hook to run when transcription starts.";
                      };
                      on_transcription_receive = mkOption {
                        type = types.nullOr hookType;
                        default = null;
                        description = "Hook to run when transcription is received.";
                      };
                      on_transcription_stop = mkOption {
                        type = types.nullOr hookType;
                        default = null;
                        description = "Hook to run when transcription stops.";
                      };
                    };
                  });
                  default = null;
                  description = "Hooks to run at different stages of transcription.";
                };
                vad_config = mkOption {
                  type = types.nullOr vadConfigType;
                  default = null;
                  description = "Voice Activity Detection (VAD) configuration for this profile.";
                };
              };
            };
          in
          {
            openai_api_key = mkOption {
              type = types.nullOr types.str;
              default = null;
              description = "Your OpenAI API key.";
            };

            openai_base_url = mkOption {
              type = types.nullOr types.str;
              default = null;
              description = "The base URL for the OpenAI API.";
            };

            audio_buffer_duration_seconds = mkOption {
              type = types.nullOr types.ints.unsigned;
              default = null;
              description = "The duration of the audio buffer in seconds.";
            };

            audio_sample_rate = mkOption {
              type = types.nullOr types.ints.unsigned;
              default = null;
              description = "The sample rate of the audio.";
            };

            audio_channels = mkOption {
              type = types.nullOr types.ints.unsigned;
              default = null;
              description = "The number of audio channels.";
            };

            whisper_model = mkOption {
              type = types.nullOr types.str;
              default = null;
              description = "The Whisper model to use.";
            };

            whisper_language = mkOption {
              type = types.nullOr types.str;
              default = null;
              description = "The language for Whisper transcription.";
            };

            whisper_timeout_seconds = mkOption {
              type = types.nullOr types.ints.unsigned;
              default = null;
              description = "The timeout for Whisper transcription in seconds.";
            };

            whisper_max_retries = mkOption {
              type = types.nullOr types.ints.unsigned;
              default = null;
              description = "The maximum number of retries for Whisper transcription.";
            };

            realtime_model = mkOption {
              type = types.nullOr types.str;
              default = null;
              description = "The real-time model to use.";
            };

            rust_log = mkOption {
              type = types.nullOr types.str;
              default = null;
              description = "The log level for Rust.";
            };

            enable_audio_feedback = mkOption {
              type = types.nullOr types.bool;
              default = null;
              description = "Whether to enable audio feedback.";
            };

            beep_volume = mkOption {
              type = types.nullOr types.float;
              default = null;
              description = "The volume of the audio feedback beep.";
            };

            profiles = mkOption {
              type = types.attrsOf profileType;
              default = { };
              description = "Transcription profiles.";
            };
          };

        systemdService = {
          enable = mkEnableOption "Hotline systemd service";

          environmentFile = mkOption {
            type = types.nullOr types.path;
            default = null;
            description = "Path to a file with environment variables to load.";
          };
        };
      };

      config = mkIf cfg.enable {
        home.packages = [ cfg.package ];

        xdg.configFile."hotline/hotline.toml" = {
          source = (pkgs.formats.toml { }).generate "hotline.toml" finalSettings;
        };

        systemd.user.services.hotline = mkIf cfg.systemdService.enable {
          Unit = {
            Description = "Hotline daemon";
            After = [ "sound.target" ];
            Requires = [ "sound.target" ];
          };

          Service = lib.filterAttrs (_: val: val != null) {
            ExecStart = "${cfg.package}/bin/hotline-daemon";
            Restart = "on-failure";
            EnvironmentFile = cfg.systemdService.environmentFile;
          };

          Install = {
            WantedBy = [ "default.target" ];
          };
        };
      };
    };
}