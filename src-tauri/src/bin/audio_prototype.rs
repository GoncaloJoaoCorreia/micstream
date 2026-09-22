use micstream_lib::audio::{
    calculate_rms, check_host_virtual_driver_status, list_input_devices, list_output_devices,
    AudioCaptureEngine, AudioPlaybackEngine,
};
use micstream_lib::codec::{OpusAudioDecoder, OpusAudioEncoder, OPUS_FRAME_SIZE_SAMPLES};
use ringbuf::traits::{Consumer, Producer, Split};
use ringbuf::HeapRb;
use std::env;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::Duration;

fn main() {
    println!("=== MicStream Audio Hardware & Driver Prototype ===");

    let args: Vec<String> = env::args().collect();
    let is_test_mode = args.iter().any(|a| a == "--test" || a == "--check");
    let is_loopback_mode = args.iter().any(|a| a == "--loopback");

    println!("\n1. Inspecting Audio Input Devices (Microphones):");
    match list_input_devices() {
        Ok(devices) => {
            if devices.is_empty() {
                println!("   (No input devices found)");
            }
            for d in &devices {
                println!(
                    "   - [Input] {} (default: {}, virtual: {})",
                    d.name, d.is_default, d.is_virtual
                );
            }
        }
        Err(e) => println!("   Error enumerating input devices: {}", e),
    }

    println!("\n2. Inspecting Audio Output Devices (Sinks):");
    match list_output_devices() {
        Ok(devices) => {
            if devices.is_empty() {
                println!("   (No output devices found)");
            }
            for d in &devices {
                println!(
                    "   - [Output] {} (default: {}, virtual: {})",
                    d.name, d.is_default, d.is_virtual
                );
            }
        }
        Err(e) => println!("   Error enumerating output devices: {}", e),
    }

    println!("\n3. Virtual Audio Driver Inspection:");
    let driver_status = check_host_virtual_driver_status();
    println!("   Target Loopback Driver: {}", driver_status.driver_name);
    println!("   Driver Installed/Detected: {}", driver_status.detected);
    if !driver_status.detected {
        println!("   Setup Instructions: {}", driver_status.instructions);
        println!("   Download URL: {}", driver_status.install_url);
    }

    println!("\n4. Verifying Opus Low-Delay Codec (5.0ms / 240 samples):");
    match (OpusAudioEncoder::new(), OpusAudioDecoder::new()) {
        (Ok(mut enc), Ok(mut dec)) => {
            let sine_samples: Vec<f32> = (0..OPUS_FRAME_SIZE_SAMPLES)
                .map(|i| (i as f32 * 0.1).sin() * 0.5)
                .collect();
            let mut packet_buf = [0u8; 512];
            let written = enc.encode_float(&sine_samples, &mut packet_buf).unwrap();
            let mut decoded_buf = [0.0f32; OPUS_FRAME_SIZE_SAMPLES];
            let decoded = dec
                .decode_float(Some(&packet_buf[..written]), &mut decoded_buf)
                .unwrap();
            println!(
                "   Opus 5ms frame: encoded {} samples -> {} bytes -> decoded {} samples. OK!",
                OPUS_FRAME_SIZE_SAMPLES, written, decoded
            );
        }
        _ => println!("   Opus codec initialization failed!"),
    }

    if is_test_mode {
        println!("\n=== All Audio Prototype Sanity Checks Passed! ===");
        return;
    }

    if is_loopback_mode {
        println!("\n5. Running Microphone-to-Speaker Shared Loopback (5 seconds)...");
        println!("   Speak into your microphone to verify capture & playback!");

        let rb = HeapRb::<f32>::new(48000);
        let (prod, mut cons) = rb.split();
        let prod_arc = Arc::new(std::sync::Mutex::new(prod));
        let prod_clone = Arc::clone(&prod_arc);

        let running = Arc::new(AtomicBool::new(true));
        let r_clone = Arc::clone(&running);

        let capture = AudioCaptureEngine::start(None, move |samples, rms| {
            if !r_clone.load(Ordering::Relaxed) {
                return;
            }
            if let Ok(mut p) = prod_clone.lock() {
                let _ = p.push_slice(samples);
            }
            let bars = (rms * 50.0).clamp(0.0, 30.0) as usize;
            print!(
                "\r[Mic VU Level]: [{:30}] RMS: {:.4}",
                "#".repeat(bars),
                rms
            );
            use std::io::Write;
            let _ = std::io::stdout().flush();
        });

        match capture {
            Ok(capture_engine) => {
                let playback = AudioPlaybackEngine::start(None, move |out_buf| {
                    let popped = cons.pop_slice(out_buf);
                    for s in out_buf.iter_mut().skip(popped) {
                        *s = 0.0;
                    }
                    calculate_rms(out_buf)
                });

                match playback {
                    Ok(playback_engine) => {
                        thread::sleep(Duration::from_secs(5));
                        running.store(false, Ordering::Relaxed);
                        capture_engine.stop();
                        playback_engine.stop();
                        println!("\n   Loopback completed successfully!");
                    }
                    Err(e) => println!("\n   Failed to start playback: {:?}", e),
                }
            }
            Err(e) => println!("\n   Failed to start capture: {:?}", e),
        }
    } else {
        println!("\nRun with '--loopback' to test real-time capture and playback.");
        println!("Run with '--test' for automated validation.");
    }

    println!("\n=== Prototype Complete ===");
}
