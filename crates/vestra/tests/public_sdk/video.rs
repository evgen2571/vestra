use super::*;

#[test]
fn public_sdk_prepares_and_renders_a_complete_audio_project() {
    let directory = tempdir().expect("temporary directory");
    let project = vestra::Project::from_json(
        r##"{"schema_version":1,"output":{"path":"unused.mp4","width":16,"height":16,"frame_rate":"30/1","background":"#000000","quality":"preview","audio":true,"duration_mode":"explicit","duration":2},"assets":[{"id":"tone","type":"audio","source":"assets/fixtures/tone.wav"}],"audio":{"tracks":[{"id":"music","gain":0.8,"clips":[{"id":"outgoing","asset":"tone","start":0,"trim_start":0,"trim_end":1.5,"fade_out":0.5,"fade_out_curve":"equal_power"},{"id":"incoming","asset":"tone","start":1,"trim_start":0,"trim_end":1.5,"fade_in":0.5,"fade_in_curve":"equal_power","gain_automation":{"keyframes":[{"time":0,"gain":0,"interpolation":"linear"},{"time":0.25,"gain":1,"interpolation":"hold"}]}}]},{"id":"ambience","gain":0.2,"clips":[{"id":"bed","asset":"tone","start":0,"trim_start":0,"trim_end":2}]}]},"visual":{"clips":[{"id":"canvas","source":{"type":"solid_color","colour":"#000000"},"start":0,"duration":2,"layer":0,"opacity":{"base_value":1}}]}}"##,
        fixture("."),
    )
    .expect("schema-v1 audio project");
    let editor = Editor::new();
    assert!(editor.validate(&project).is_valid());
    let inspection = editor.inspect(&project, false).expect("inspect audio");
    let audio = inspection.audio.expect("authored audio stays inspectable");
    assert_eq!(audio.track_count, 2);
    assert_eq!(
        audio.tracks[0].clips[1].gain_automation[1].interpolation,
        vestra::AudioGainInterpolation::Hold
    );
    let mut prepared = editor
        .prepare(&project, PrepareOptions::new(BackendPreference::Cpu))
        .expect("prepare CPU");
    let result = prepared
        .render_video(
            PreparedVideoRenderRequest::new(directory.path().join("audio.mp4"))
                .with_overwrite(true),
            &mut |_| {},
            &CancellationToken::new(),
        )
        .expect("render audio");
    assert!(result.audio_present);
    assert!(result.output.is_file());
}

#[test]
fn public_sdk_prepares_canonical_audio_reactive_video_without_output_audio() {
    let directory = tempdir().expect("temporary directory");
    let project = vestra::Project::from_json(
        r##"
        {
          "schema_version": 1,
          "output": {"path": "unused.mp4", "width": 16, "height": 16, "frame_rate": "30/1", "background": "#000000", "quality": "preview", "audio": false, "duration_mode": "explicit", "duration": 1},
          "assets": [{"id": "tone", "type": "audio", "source": "assets/fixtures/tone.wav"}],
          "audio": {"tracks": [{"id": "music", "clips": [{"id": "tone-clip", "asset": "tone", "start": 0, "trim_start": 0, "trim_end": 1}]}]},
          "visual": {"clips": [{
            "id": "canvas", "source": {"type": "solid_color", "colour": "#000000"}, "start": 0, "duration": 1, "layer": 0,
            "opacity": {"base_value": 1},
            "effects": [{"id": "reactive-brightness", "type": "brightness", "amount": {
              "base_value": 0,
              "modifiers": [{"operation": "add", "signal": {
                "source": {"type": "audio", "tap": "master", "feature": {"type": "band_energy", "min_hz": 40, "max_hz": 160}},
                "transforms": [
                  {"type": "gain", "gain": 2},
                  {"type": "remap", "input_min": 0, "input_max": 1, "output_start": 0, "output_end": 1},
                  {"type": "clamp", "min": 0, "max": 1},
                  {"type": "envelope", "attack": 0.02, "release": 0.18},
                  {"type": "response_curve", "x1": 0.42, "y1": 0, "x2": 0.58, "y2": 1}
                ]
              }}]
            }}]
          }]}
        }
        "##,
        fixture("."),
    )
    .expect("canonical audio-reactive project");
    let serialized = project.to_json().expect("serialize signal project");
    assert!(serialized.contains("\"modifiers\""));
    vestra::Project::from_json(&serialized, fixture("."))
        .expect("reload serialized signal project");
    let editor = Editor::new();
    assert!(editor.validate(&project).is_valid());
    let mut prepared = editor
        .prepare(&project, PrepareOptions::new(BackendPreference::Cpu))
        .expect("prepare audio-reactive CPU project");
    let frame = prepared
        .render_frame(Duration::ZERO)
        .expect("evaluate audio-reactive frame");
    assert_eq!(frame.width(), 16);
    assert_eq!(frame.height(), 16);
    let result = prepared
        .render_video(
            PreparedVideoRenderRequest::new(directory.path().join("reactive.mp4"))
                .with_overwrite(true),
            &mut |_| {},
            &CancellationToken::new(),
        )
        .expect("render video without output audio");
    assert!(!result.audio_present);
    assert!(result.output.is_file());
}

#[test]
fn prepared_cpu_video_counters_exclude_previous_operations() {
    let directory = tempdir().expect("temporary directory");
    // Warm every auto-selected worker before checking decoder reuse in the next operation.
    let total_frames =
        u64::try_from(std::thread::available_parallelism().map_or(1, std::num::NonZeroUsize::get))
            .expect("CPU count fits u64")
            .max(2);
    let status = std::process::Command::new("ffmpeg")
        .args([
            "-v",
            "error",
            "-f",
            "lavfi",
            "-i",
            "testsrc2=s=16x16:r=4:d=1",
            "-c:v",
            "libx264",
            "-pix_fmt",
            "yuv420p",
        ])
        .arg(directory.path().join("clip.mp4"))
        .status()
        .expect("generate video fixture");
    assert!(status.success());
    let project = vestra::Project::from_value(
        serde_json::json!({
            "schema_version": 1,
            "output": {"path": "unused.mp4", "width": 16, "height": 16,
                "frame_rate": total_frames, "background": "#000000", "quality": "preview",
                "audio": false, "duration_mode": "explicit", "duration": 1},
            "assets": [{"id": "video", "type": "video", "source": "clip.mp4"}],
            "visual": {"clips": [{"id": "clip", "source": {"type": "video", "asset": "video"},
                "start": 0, "duration": 1, "layer": 0, "opacity": {"base_value": 1}}]}
        }),
        directory.path(),
    )
    .expect("project");
    let mut prepared = Editor::new()
        .prepare(&project, PrepareOptions::new(BackendPreference::Cpu))
        .expect("prepare video project");
    let mut results = Vec::new();
    for name in ["first.mp4", "second.mp4"] {
        results.push(
            prepared
                .render_video(
                    PreparedVideoRenderRequest::new(directory.path().join(name)),
                    |_| {},
                    &CancellationToken::new(),
                )
                .expect("render prepared video"),
        );
    }
    for result in &results {
        assert_eq!(result.total_frames, total_frames);
        assert_eq!(result.render_backend, "cpu");
        assert_eq!(result.performance.video_frame_requests, total_frames);
        assert_eq!(
            result.performance.video_cache_hits + result.performance.video_cache_misses,
            total_frames
        );
    }
    let first = &results[0].performance;
    let second = &results[1].performance;
    assert!(first.video_actual_decodes > 0);
    assert!(second.video_actual_decodes <= first.video_actual_decodes);
    assert!(first.video_decoder_open_count > 0);
    assert_eq!(first.video_decoder_open_count, first.pipeline_depth as u64);
    assert_eq!(second.video_decoder_open_count, 0);
    assert_eq!(first.video_decoder_session_count, first.pipeline_depth);
    assert_eq!(second.video_decoder_session_count, first.pipeline_depth);
}

#[test]
fn prepared_cpu_video_operation_uses_an_operation_request() {
    let directory = tempdir().expect("temporary directory");
    let project_path = fixture("tests/fixtures/wgpu-small-rgba.json");
    let project = Editor::new().load_project(project_path).expect("project");
    let mut prepared = Editor::new()
        .prepare(&project, PrepareOptions::new(BackendPreference::Cpu))
        .expect("prepare");
    let output = directory.path().join("prepared.mp4");
    let result = prepared
        .render_video(
            PreparedVideoRenderRequest::new(&output).with_overwrite(true),
            |_| {},
            &CancellationToken::new(),
        )
        .expect("render");
    assert_eq!(result.output, output);
    assert_eq!(
        result.timing_scope,
        vestra::RenderTimingScope::PreparedOperation
    );
    assert_eq!(result.elapsed_ms, result.timings.operation_total_ms);
    assert!(prepared.render_frame_number(0).is_ok());
}

#[test]
fn preflight_video_timing_preserves_nested_locations_and_clip_identity() {
    use serde_json::json;

    let directory = tempdir().expect("temporary directory");
    let status = std::process::Command::new("ffmpeg")
        .args([
            "-v",
            "error",
            "-f",
            "lavfi",
            "-i",
            "color=c=red:s=2x2:r=10:d=1",
            "-c:v",
            "libx264",
            "-pix_fmt",
            "yuv420p",
        ])
        .arg(directory.path().join("clip.mp4"))
        .status()
        .expect("generate video fixture");
    assert!(status.success());

    let video = |id: &str| {
        json!({
            "id": id, "source": {"type": "video", "asset": "video"},
            "start": 0.5, "duration": 0.25, "layer": 0, "opacity": {"base_value": 1.0},
            "source_start": 0.5, "playback_rate": 2.0
        })
    };
    let mut value = json!({
        "schema_version": 1,
        "output": {"path": "out.mp4", "width": 2, "height": 2,
            "frame_rate": "30000/1001", "background": "#000000",
            "quality": "preview", "audio": false,
            "duration_mode": "explicit", "duration": 10},
        "assets": [{"id": "video", "type": "video", "source": "clip.mp4"}],
        "visual": {"clips": [video("root-video"), {
            "id": "outer", "start": 3, "duration": 5, "layer": 0, "opacity": {"base_value": 1.0},
            "source": {"type": "group", "clips": [video("outer-video"), {
                "id": "inner", "start": 1, "duration": 2, "layer": 0, "opacity": {"base_value": 1.0},
                "source": {"type": "group", "clips": [
                    video("valid-sibling"), video("inner-first"), video("inner-second")
                ]}
            }]}
        }]}
    });
    let editor = Editor::new();
    let project = vestra::Project::from_value(value.clone(), directory.path()).expect("project");
    let report = editor.preflight(&project, PreflightOptions::for_inspection());
    assert!(
        report.is_valid(),
        "source end is inclusive: {:?}",
        report.diagnostics()
    );

    for (pointer, id, source_start) in [
        ("/visual/clips/0", "root-video", 0.75),
        ("/visual/clips/1/source/clips/0", "outer-video", 0.75),
        (
            "/visual/clips/1/source/clips/1/source/clips/1",
            "inner-first",
            0.75,
        ),
        (
            "/visual/clips/1/source/clips/1/source/clips/2",
            "inner-second",
            1.0,
        ),
    ] {
        value.pointer_mut(pointer).expect("clip")["source_start"] = json!(source_start);
        let project =
            vestra::Project::from_value(value.clone(), directory.path()).expect("project");
        assert!(editor.validate(&project).is_valid());
        let report = editor.preflight(&project, PreflightOptions::for_inspection());
        assert!(!report.is_valid());
        let diagnostics = report.diagnostics();
        assert_eq!(diagnostics.len(), 1, "{diagnostics:?}");
        let diagnostic = &diagnostics[0];
        assert_eq!(diagnostic.code, "VESTRA-VIDEO-TIMING");
        assert_eq!(diagnostic.category, vestra::Category::Semantic);
        assert_eq!(diagnostic.severity, vestra::Severity::Fatal);
        assert_eq!(diagnostic.pointer.as_deref(), Some(pointer));
        assert_eq!(value.pointer(pointer).expect("owning clip")["id"], id);
        assert!(diagnostic.message.contains(&format!("'{id}'")));
        value.pointer_mut(pointer).expect("clip")["source_start"] = json!(0.5);
    }
}
