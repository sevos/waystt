# Example home-manager configuration for HotLine
# This shows how to configure HotLine using the home-manager module
# Based on the hotline.toml.example file

{
  programs.hotline = {
    enable = true;
    
    settings = {
      # OpenAI API configuration
      openai_api_key = "your-api-key-here";  # Or use environment variable
      # openai_base_url = "https://api.openai.com";  # Optional: custom API endpoint
      
      # Real-time transcription model
      # Options: "whisper-1", "gpt-4o-transcribe", "gpt-4o-mini-transcribe"
      realtime_model = "whisper-1";
      
      # Audio configuration
      audio_buffer_duration_seconds = 300;  # 5 minutes
      audio_sample_rate = 16000;  # Optimized for Whisper
      audio_channels = 1;  # Mono
      
      # Whisper transcription settings
      whisper_model = "whisper-1";
      whisper_language = "auto";  # or specify like "en", "es", etc.
      whisper_timeout_seconds = 60;
      whisper_max_retries = 3;
      
      # Audio feedback settings
      enable_audio_feedback = true;
      beep_volume = 0.1;  # Range: 0.0 to 1.0
      
      # Logging
      rust_log = "info";  # Options: trace, debug, info, warn, error
      
      # Transcription profiles
      # Define named profiles for different transcription scenarios
      profiles = {
        default = {
          model = "whisper-1";
          language = "en";
        };
        
        coding = {
          model = "whisper-1";
          language = "en";
          prompt = "The user is a programmer, so expect technical terms.";
          
          # Example hooks for lifecycle events (uncomment to enable)
          # hooks = {
          #   on_transcription_start = {
          #     type = "spawn";
          #     command = [ "notify-send" "Recording started" ];
          #   };
          #   on_transcription_receive = {
          #     type = "spawn_with_stdin";  # Text is piped to stdin
          #     command = [ "wl-copy" ];
          #   };
          #   on_transcription_stop = {
          #     type = "spawn";
          #     command = [ "notify-send" "Recording stopped" ];
          #   };
          # };
        };
        
        spanish = {
          model = "whisper-1";
          language = "es";
          prompt = "El usuario habla español.";
        };
        
        coding-spanish = {
          model = "gpt-4o-mini-transcribe";  # Using GPT-4o mini transcribe instead of whisper
          language = "es";
          prompt = "El usuario es un programador escribiendo código. Espera términos técnicos de programación, nombres de funciones, variables en inglés mezclados con español.";
          
          # Semantic VAD configuration for better context understanding
          vad_config = {
            SemanticVad = {
              eagerness = "medium";
            };
          };
          
          # Example hook to type the transcribed text using xdotool (uncomment to enable)
          # hooks = {
          #   on_transcription_receive = {
          #     type = "spawn_with_stdin";
          #     command = [ "xdotool" "type" "--file" "-" ];
          #   };
          # };
        };
        
        meeting = {
          model = "whisper-1";
          language = "auto";
          prompt = "This is a business meeting with multiple speakers.";
          
          # Example hooks for meeting transcription with notifications (uncomment to enable)
          # hooks = {
          #   on_transcription_start = {
          #     type = "spawn";
          #     command = [ "sh" "-c" "echo 'Meeting started at $(date)' >> /tmp/meeting_log.txt" ];
          #   };
          #   on_transcription_receive = {
          #     type = "spawn_with_stdin";
          #     command = [ "tee" "-a" "/tmp/meeting_transcript.txt" ];
          #   };
          #   on_transcription_stop = {
          #     type = "spawn";
          #     command = [ "sh" "-c" "echo 'Meeting ended at $(date)' >> /tmp/meeting_log.txt" ];
          #   };
          # };
        };
        
        # Complete example with all hooks configured
        full-example = {
          model = "whisper-1";
          language = "en";
          prompt = "General transcription with full lifecycle hooks";
          
          # Hook executed when transcription starts
          hooks = {
            on_transcription_start = {
              type = "spawn";
              command = [ "notify-send" "HotLine" "Recording started" ];
            };
            # Hook executed when transcription text is received
            on_transcription_receive = {
              type = "spawn_with_stdin";  # Text is piped to stdin
              command = [ "wl-copy" ];  # Copy to clipboard
            };
            # Hook executed when transcription stops
            on_transcription_stop = {
              type = "spawn";
              command = [ "notify-send" "HotLine" "Recording stopped" ];
            };
          };
        };
      };
    };
    
    # Optional: Enable systemd service
    systemdService = {
      enable = true;
      # environmentFile = "/path/to/.env";  # Optional: path to environment file with OPENAI_API_KEY
    };
  };
}

# Minimal configuration example:
# {
#   programs.hotline = {
#     enable = true;
#     settings = {
#       openai_api_key = "your-api-key-here";
#     };
#   };
# }

# With environment file for API key:
# {
#   programs.hotline = {
#     enable = true;
#     systemdService = {
#       enable = true;
#       environmentFile = "/home/user/.config/hotline/.env";
#     };
#   };
# }

# With environment variables directly in the service:
# {
#   programs.hotline = {
#     enable = true;
#     systemdService = {
#       enable = true;
#       environment = {
#         OPENAI_API_KEY = "sk-your-api-key-here";
#         RUST_LOG = "debug";
#       };
#     };
#   };
# }