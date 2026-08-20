# Python problems

Build/import failures usually mean the native extension or FFmpeg development dependencies are missing. Install with the repository's locked `uv` environment or the documented source-build path, then check `uv run python -c "import vestra"`.

If static typing and runtime disagree, confirm the installed package revision and inspect `python/vestra/_native.pyi`. `Project.validate()` does not probe files; use preparation/rendering to surface asset and media failures. Progress callback exceptions stop rendering, and cancellation raises `CancelledError` rather than returning `RenderResult`.
