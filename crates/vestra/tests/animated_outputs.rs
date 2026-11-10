use std::time::Duration;

use vestra::{
    BackendPreference, CancellationToken, Editor, PrepareOptions, PreparedVideoRenderRequest,
};

#[test]
fn static_animations_render_one_frame_and_stream_cached_pixels_for_full_and_selected_outputs() {
    let directory = tempfile::tempdir().unwrap();
    let project = vestra::Project::from_json(
        r##"{"schema_version":1,"output":{"path":"unused.mp4","width":16,"height":16,"frame_rate":"10/1","background":"#204080","quality":"preview","audio":false,"duration_mode":"explicit","duration":0.3},"assets":[],"visual":{"clips":[]}}"##,
        directory.path(),
    ).unwrap();
    for extension in ["gif", "webp"] {
        for selected in [false, true] {
            let mut prepared = Editor::new()
                .prepare(&project, PrepareOptions::new(BackendPreference::Cpu))
                .unwrap();
            let mut request = PreparedVideoRenderRequest::new(
                directory
                    .path()
                    .join(format!("static-{selected}.{extension}")),
            );
            let frames = if selected {
                request = request
                    .with_range(Duration::from_millis(100), Some(Duration::from_millis(300)));
                2
            } else {
                3
            };
            let result = prepared
                .render_video(request, |_| {}, &CancellationToken::new())
                .unwrap();
            assert_eq!(result.total_frames, frames);
            assert_eq!(
                result
                    .performance
                    .static_visual_frame_cache_population_renders,
                1
            );
            assert_eq!(
                result.performance.static_visual_frame_cache_hits,
                frames - 1
            );
            assert!(!result.performance.static_visual_ffmpeg_fast_path_used);
            assert_eq!(
                result.performance.encoder_video_input_mode,
                "raw_rgba_frames"
            );
            assert_eq!(
                result.performance.encoder_video_frames_pushed_from_rust,
                frames
            );
            assert!(!result.audio_present);
            assert!(result.output.exists());
        }
    }
}
