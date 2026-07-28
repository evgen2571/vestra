use video_editor::RenderEvent;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ProgressFormat {
    Human,
    Json,
    None,
}

pub fn write_progress(format: ProgressFormat, event: &RenderEvent) {
    match format {
        ProgressFormat::None => {}
        ProgressFormat::Json => match serde_json::to_string(event) {
            Ok(value) => println!("{value}"),
            Err(error) => eprintln!("warning: cannot serialize progress event: {error}"),
        },
        ProgressFormat::Human => match event.progress {
            Some(progress) => eprintln!(
                "{}: {}/{} ({:.0}%)",
                event.kind,
                event.frame,
                event.total_frames,
                progress * 100.0
            ),
            None => eprintln!("{}: {}/{}", event.kind, event.frame, event.total_frames),
        },
    }
}
