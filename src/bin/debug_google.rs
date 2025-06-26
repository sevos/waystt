// Debug tool to test Google Cloud Speech API connection
use google_api_proto::google::cloud::speech::v2::{
    recognition_config::DecodingConfig, recognize_request::AudioSource,
    speech_client::SpeechClient, AutoDetectDecodingConfig, RecognitionConfig, RecognitionFeatures,
    RecognizeRequest,
};
use tonic::Request;
use yup_oauth2::{ServiceAccountAuthenticator, ServiceAccountKey};
use std::error::Error;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("🔍 Debug: Testing Google Cloud Speech API connection...");
    
    let credentials_path = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "/home/sevos/.config/waystt/google-key.json".to_string());
    
    println!("📂 Debug: Reading credentials from: {}", credentials_path);
    
    // Read service account key
    let service_account_key = tokio::fs::read_to_string(&credentials_path).await?;
    println!("✅ Debug: Credentials file read successfully");
    
    let service_account_key: ServiceAccountKey = serde_json::from_str(&service_account_key)?;
    println!("✅ Debug: Credentials parsed successfully");
    
    let project_id = service_account_key.project_id.clone()
        .ok_or("No project_id in service account key")?;
    println!("🏗️  Debug: Project ID: {}", project_id);
    
    // Create authenticator
    println!("🔐 Debug: Creating authenticator...");
    let auth = ServiceAccountAuthenticator::builder(service_account_key)
        .build()
        .await?;
    println!("✅ Debug: Authenticator created successfully");
    
    // Get access token
    println!("🎫 Debug: Getting access token...");
    let token = auth
        .token(&["https://www.googleapis.com/auth/cloud-platform"])
        .await?;
    println!("✅ Debug: Access token obtained successfully");
    
    // Test connection with detailed diagnostics
    println!("🌐 Debug: Testing HTTPS connection to speech.googleapis.com...");
    
    // Create the channel with explicit TLS configuration
    let tls_config = tonic::transport::ClientTlsConfig::new()
        .domain_name("speech.googleapis.com");
    let endpoint = tonic::transport::Channel::from_static("https://speech.googleapis.com")
        .tls_config(tls_config)?;
    let channel_result = endpoint.connect().await;
    
    match channel_result {
        Ok(channel) => {
            println!("✅ Debug: HTTPS connection successful");
            
            // Create client
            let client = SpeechClient::new(channel);
            let auth_token = format!("Bearer {}", token.token().unwrap_or(""));
            println!("🤖 Debug: Client created with auth token");
            
            // Try a minimal request
            let parent = format!("projects/{}/locations/global", project_id);
            println!("📍 Debug: Using parent: {}", parent);
            
            // Create minimal WAV audio data (44-byte header + minimal data)
            let wav_header = vec![
                0x52, 0x49, 0x46, 0x46, // "RIFF"
                0x28, 0x00, 0x00, 0x00, // File size (40 bytes)
                0x57, 0x41, 0x56, 0x45, // "WAVE"
                0x66, 0x6D, 0x74, 0x20, // "fmt "
                0x10, 0x00, 0x00, 0x00, // Size of fmt chunk (16)
                0x01, 0x00,             // Audio format (1 = PCM)
                0x01, 0x00,             // Number of channels (1)
                0x80, 0x3E, 0x00, 0x00, // Sample rate (16000)
                0x00, 0x7D, 0x00, 0x00, // Byte rate (16000 * 1 * 2)
                0x02, 0x00,             // Block align (1 * 2)
                0x10, 0x00,             // Bits per sample (16)
                0x64, 0x61, 0x74, 0x61, // "data"
                0x08, 0x00, 0x00, 0x00, // Data size (8 bytes)
                0x00, 0x00, 0x00, 0x00, // Audio data (silence)
                0x00, 0x00, 0x00, 0x00, // Audio data (silence)
            ];
            
            let config = RecognitionConfig {
                decoding_config: Some(DecodingConfig::AutoDecodingConfig(AutoDetectDecodingConfig {})),
                model: "latest_short".to_string(),
                language_codes: vec!["en-US".to_string()],
                features: Some(RecognitionFeatures {
                    enable_automatic_punctuation: true,
                    enable_word_time_offsets: false,
                    enable_word_confidence: false,
                    ..Default::default()
                }),
                adaptation: None,
                transcript_normalization: None,
                translation_config: None,
            };
            
            let request = RecognizeRequest {
                recognizer: format!("{}/recognizers/_", parent),
                config: Some(config),
                config_mask: None,
                audio_source: Some(AudioSource::Content(wav_header.into())),
            };
            
            println!("📤 Debug: Sending test request...");
            let mut client = client.clone();
            let mut req = Request::new(request);
            req.metadata_mut().insert(
                "authorization",
                auth_token.parse().unwrap(),
            );
            
            let response = client.recognize(req).await;
            match response {
                Ok(resp) => {
                    let inner = resp.into_inner();
                    println!("🎉 Debug: Request successful!");
                    println!("📋 Debug: Results count: {}", inner.results.len());
                    if let Some(result) = inner.results.first() {
                        println!("📝 Debug: Alternatives count: {}", result.alternatives.len());
                        if let Some(alt) = result.alternatives.first() {
                            println!("💬 Debug: Transcript: '{}'", alt.transcript);
                        }
                    }
                }
                Err(e) => {
                    println!("❌ Debug: Request failed with error: {}", e);
                    println!("🔍 Debug: Error details: {:?}", e);
                    
                    // Check the gRPC status code
                    println!("📊 Debug: gRPC status code: {:?}", e.code());
                    
                    // Check the source of the error
                    if let Some(source) = e.source() {
                        println!("🔗 Debug: Error source: {}", source);
                    }
                }
            }
        }
        Err(e) => {
            println!("❌ Debug: HTTPS connection failed: {}", e);
            println!("🔍 Debug: Error details: {:?}", e);
            
            // Check if it's a transport error
            if let Some(source) = e.source() {
                println!("🔗 Debug: Error source: {}", source);
            }
        }
    }
    
    println!("✨ Debug: Test completed");
    Ok(())
}