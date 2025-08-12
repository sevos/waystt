use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

fn main() {
    println!("Playing answering machine beep (1kHz, 800ms)...");

    let host = cpal::default_host();
    let device = host.default_output_device().expect("No output device");
    let config = device.default_output_config().expect("No default config");

    let sample_rate = config.sample_rate().0 as f32;
    let channels = config.channels() as usize;

    let frequency = 1000.0; // 1kHz
    let duration_ms = 800.0; // 800ms
    let volume = 0.3; // 30% volume
    let sample_count = (sample_rate * duration_ms / 1000.0) as usize;

    let playing = Arc::new(AtomicBool::new(true));
    let playing_clone = playing.clone();

    let mut sample_index = 0usize;
    let mut phase = 0.0f32;

    let stream = device
        .build_output_stream(
            &config.into(),
            move |data: &mut [f32], _: &cpal::OutputCallbackInfo| {
                for frame in data.chunks_mut(channels) {
                    if sample_index >= sample_count {
                        playing_clone.store(false, Ordering::Relaxed);
                        for sample in frame {
                            *sample = 0.0;
                        }
                        continue;
                    }

                    let sample_value = (phase * 2.0 * std::f32::consts::PI).sin() * volume;

                    for sample in frame {
                        *sample = sample_value;
                    }

                    phase += frequency / sample_rate;
                    if phase > 1.0 {
                        phase -= 1.0;
                    }

                    sample_index += 1;
                }
            },
            |err| eprintln!("Stream error: {err}"),
            None,
        )
        .expect("Failed to build stream");

    stream.play().expect("Failed to play stream");

    while playing.load(Ordering::Relaxed) {
        std::thread::sleep(Duration::from_millis(10));
    }

    println!("Beep complete!");
}
