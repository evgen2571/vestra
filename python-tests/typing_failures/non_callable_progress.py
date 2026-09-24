import vestra


project: vestra.ProjectSnapshot
prepared: vestra.PreparedProject
prepared_request: vestra.PreparedVideoRenderRequest
render_request: vestra.RenderRequest

prepared.render_video(prepared_request, on_progress=123)  # E: Argument "on_progress" to "render_video" of "PreparedProject" has incompatible type "int"; expected "Callable[[RenderEvent], object] | None"
vestra.Editor().render(project, render_request, on_progress="not callable")  # E: Argument "on_progress" to "render" of "Editor" has incompatible type "str"; expected "Callable[[RenderEvent], object] | None"
