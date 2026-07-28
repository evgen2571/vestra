use video_editor::{BackendPreference, CancellationToken, Editor, RenderRequest};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let editor = Editor::new();
    let project = "project.json";
    let project = editor.load_project(project)?;
    let result = editor.render(
        &project,
        RenderRequest {
            output: Some("result.mp4".into()),
            overwrite: true,
            preview: false,
            backend: BackendPreference::Auto,
        },
        &mut |event| println!("{}", event.kind),
        &CancellationToken::new(),
    )?;
    println!("{}", result.output.display());
    Ok(())
}
