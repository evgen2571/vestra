use std::{
    sync::{Arc, atomic::AtomicBool},
    time::Instant,
};

use video_editor::application::{RenderRequest, render_project};

fn main() {
    let output = tempfile::tempdir().expect("temporary benchmark directory");
    let output_path = output.path().join("animation-effects-v2.mp4");
    let started = Instant::now();
    let result = render_project(
        std::path::Path::new("examples/projects/animation-effects-v2.json"),
        RenderRequest {
            output_override: Some(output_path),
            overwrite: false,
            preview: false,
            cancelled: Arc::new(AtomicBool::new(false)),
        },
        &mut |_| {},
    );
    let (_, summary) = match result {
        Ok(result) => result,
        Err(_) => panic!("benchmark project renders"),
    };
    println!(
        "animation-effects-v2: total={}ms composition={}ms encode_write={}ms cache_peak={} bytes decoded_peak={} bytes wall={}ms",
        summary.timings.total_ms,
        summary.timings.frame_composition_ms,
        summary.timings.encoder_write_ms,
        summary.performance.cache_peak_bytes,
        summary.performance.peak_decoded_bytes,
        started.elapsed().as_millis(),
    );
}
