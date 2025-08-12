use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

fn main() {
    println!("Playing busy signal (480Hz + 620Hz mixed, 3 pulses)...");

    let host = cpal::default_host();
    let device = host.default_output_device().expect("No output device");
    let config = device.default_output_config().expect("No default config");

    let sample_rate = config.sample_rate().0 as f32;
    let channels = config.channels() as usize;

    let duration_ms = 1500.0; // 1.5 seconds total (3 pulses)
    let volume = 0.3; // 30% volume
    let sample_count = (sample_rate * duration_ms / 1000.0) as usize;

    let playing = Arc::new(AtomicBool::new(true));
    let playing_clone = playing.clone();

    let mut sample_index = 0usize;
    let mut phase1 = 0.0f32;
    let mut phase2 = 0.0f32;

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

                    let progress = sample_index as f32 / sample_count as f32;

                    // Busy signal pattern: 3 pulses (250ms on, 250ms off each)
                    let sample_value = if (progress < 0.167)
                        || (0.333..0.5).contains(&progress)
                        || (0.667..0.833).contains(&progress)
                    {
                        // Mix 480Hz and 620Hz
                        let signal1 = (phase1 * 2.0 * std::f32::consts::PI).sin();
                        let signal2 = (phase2 * 2.0 * std::f32::consts::PI).sin();
                        (signal1 + signal2) * 0.5 * volume
                    } else {
                        0.0 // Silence between pulses
                    };

                    for sample in frame {
                        *sample = sample_value;
                    }

                    phase1 += 480.0 / sample_rate;
                    if phase1 > 1.0 {
                        phase1 -= 1.0;
                    }

                    phase2 += 620.0 / sample_rate;
                    if phase2 > 1.0 {
                        phase2 -= 1.0;
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

    println!("Busy signal complete!");
}
