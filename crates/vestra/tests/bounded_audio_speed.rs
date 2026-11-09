use std::process::Command;

use serde_json::json;
use vestra::{
    BackendPreference, CancellationToken, Editor, PrepareOptions, PreparedVideoRenderRequest,
    Project, RenderRequest,
};

#[test]
fn slowed_early_eof_preserves_destination_and_reports_source_bounds() {
    let directory = tempfile::tempdir().expect("fixtures");
    let source = directory.path().join("short.wav");
    let generated = Command::new("ffmpeg")
        .args([
            "-v",
            "error",
            "-f",
            "lavfi",
            "-i",
            "sine=sample_rate=48000:duration=0.0625",
            "-c:a",
            "pcm_s16le",
        ])
        .arg(&source)
        .output()
        .expect("generate source");
    assert!(generated.status.success(), "{:?}", generated.stderr);
    let project = Project::from_value(
        json!({
            "schema_version":1,
            "output":{"path":"out.mp4","width":16,"height":16,"frame_rate":"8/1",
                "background":"#000000","quality":"preview","audio":true,
                "duration_mode":"explicit","duration":0.125},
            "assets":[{"id":"tone","type":"audio","source":"short.wav"}],
            "visual":{"clips":[]},
            "audio":{"tracks":[{"id":"track","clips":[{
                "id":"clip","asset":"tone","start":0,"trim_start":0,"trim_end":2,
                "effects":[{"id":"speed","type":"playback_speed","rate":0.5}]
            }]}]}
        }),
        directory.path(),
    )
    .expect("project");
    let editor = Editor::new();
    for prepared_mode in [true, false] {
        let output = directory.path().join(format!("{prepared_mode}.mp4"));
        std::fs::write(&output, b"previous output").expect("destination");
        let result = if prepared_mode {
            editor
                .prepare(&project, PrepareOptions::new(BackendPreference::Cpu))
                .expect("bounds are checked during execution")
                .render_video(
                    PreparedVideoRenderRequest::new(&output).with_overwrite(true),
                    &mut |_| {},
                    &CancellationToken::new(),
                )
        } else {
            editor.render(
                &project,
                RenderRequest {
                    output: Some(output.clone()),
                    overwrite: true,
                    backend: BackendPreference::Cpu,
                    ..RenderRequest::default()
                },
                &mut |_| {},
                &CancellationToken::new(),
            )
        };
        let error = result.expect_err("atempo output shortage cannot be hidden by padding");
        assert!(error.diagnostics().iter().any(|diagnostic| {
            diagnostic.code == "VESTRA-AUDIO-CLIP-SOURCE"
                && diagnostic.related_id.as_deref() == Some("clip")
        }));
        assert_eq!(
            std::fs::read(output).expect("destination"),
            b"previous output"
        );
        assert!(
            !std::fs::read_dir(directory.path())
                .expect("directory")
                .any(|entry| {
                    entry
                        .expect("entry")
                        .file_name()
                        .to_string_lossy()
                        .starts_with(".vestra-")
                })
        );
    }
}
