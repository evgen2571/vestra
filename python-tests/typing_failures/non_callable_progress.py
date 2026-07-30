import video_editor


project: video_editor.Project
prepared: video_editor.PreparedProject
prepared_request: video_editor.PreparedVideoRenderRequest
render_request: video_editor.RenderRequest

prepared.render_video(prepared_request, progress=123)  # E: Argument "progress" to "render_video" of "PreparedProject" has incompatible type "int"; expected "Callable[[RenderEvent], object] | None"
video_editor.Editor().render(project, render_request, progress="not callable")  # E: Argument "progress" to "render" of "Editor" has incompatible type "str"; expected "Callable[[RenderEvent], object] | None"
