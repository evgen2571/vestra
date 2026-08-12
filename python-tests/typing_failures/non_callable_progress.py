import vestra


project: vestra.Project
prepared: vestra.PreparedProject
prepared_request: vestra.PreparedVideoRenderRequest
render_request: vestra.RenderRequest

prepared.render_video(prepared_request, progress=123)  # E: Argument "progress" to "render_video" of "PreparedProject" has incompatible type "int"; expected "Callable[[RenderEvent], object] | None"
vestra.Editor().render(project, render_request, progress="not callable")  # E: Argument "progress" to "render" of "Editor" has incompatible type "str"; expected "Callable[[RenderEvent], object] | None"
