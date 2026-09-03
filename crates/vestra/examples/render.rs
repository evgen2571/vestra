use vestra::{BackendPreference, CancellationToken, Editor, RenderRequest};

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
            progress_mode: vestra::ProgressMode::Disabled,
        },
        &mut |event| println!("{event:?}"),
        &CancellationToken::new(),
    )?;
    println!("{}", result.output.display());
    Ok(())
}
