"""One timeline definition, implemented through each engine's normal public API."""

import os
import time


def workload(name, width=1920, height=1080):
    def clip(asset, start, duration, **values):
        return dict(
            asset=asset,
            start=start,
            duration=duration,
            scale=1,
            x=0.5,
            y=0.5,
            end_x=0.5,
            opacity=1,
            brightness=0,
            fade_in=0,
            ellipse=False,
            **values,
        )

    if name == "basic":
        clips = [clip("forest", 0, 10)]
        duration = 10
    elif name == "editing":
        clips = []
        for i, asset in enumerate(("forest", "valley", "tear")):
            c = clip(asset, i * 3.8, 4.4)
            c.update(
                scale=1.05, x=0.49, end_x=0.51, brightness=0.03, fade_in=0.6 if i else 0
            )
            clips.append(c)
        duration = 12
    elif name == "layered":
        a, b = clip("valley", 1, 8), clip("tear", 3, 6)
        a.update(scale=0.5, x=0.30, end_x=0.38, opacity=0.85)
        b.update(scale=0.38, x=0.72, end_x=0.66, y=0.52, opacity=0.7, ellipse=True)
        clips = [clip("forest", 0, 10), a, b]
        duration = 10
    else:
        raise ValueError(f"unknown scenario: {name}")
    return dict(
        name=name,
        width=width,
        height=height,
        fps=30,
        duration=duration,
        clips=clips,
        encoding=dict(
            codec="libx264",
            crf=23,
            preset="veryfast",
            pixel_format="yuv420p",
            audio=False,
        ),
    )


def vestra_project(definition, inputs):
    from vestra import Project, Interpolation
    from vestra.effects import Brightness
    from vestra.sources import Video, Ellipse

    p = Project(
        size=(definition["width"], definition["height"]),
        fps=30,
        duration=definition["duration"],
        output_audio=False,
    )
    for z, c in enumerate(definition["clips"]):
        layer = p.root.add(
            Video(str(inputs / f"{c['asset']}.mp4"), sizing="cover"),
            start=c["start"],
            duration=c["duration"],
            source_start=c.get("source_start", 0),
            z=z,
        )
        layer.transform.scale = c["scale"]
        layer.transform.position.keyframe(
            0, (c["x"], c["y"]), interpolation=Interpolation.LINEAR
        )
        layer.transform.position.keyframe(c["duration"], (c["end_x"], c["y"]))
        layer.opacity = c["opacity"]
        if c["fade_in"]:
            layer.opacity.keyframe(0, 0)
            layer.opacity.keyframe(c["fade_in"], c["opacity"])
        if c["brightness"]:
            layer.effects.add(Brightness(c["brightness"]))
        if c["ellipse"]:
            layer.masks.add(
                Ellipse(
                    width=definition["width"] * 0.9,
                    height=definition["height"] * 0.9,
                    fill="#ffffff",
                )
            )
    return p


def adapter_class(adapter):
    """Mirror Vestra's conservative AdapterMetadata performance classification."""
    fields = " ".join(
        str(adapter.get(k, "")) for k in ("adapter_name", "driver_name", "driver_info")
    ).lower()
    if any(
        s in fields
        for s in (
            "lavapipe",
            "llvmpipe",
            "softpipe",
            "openswr",
            "swiftshader",
            "software rasterizer",
            "microsoft basic render driver",
            "warp",
        )
    ):
        return "software"
    name = adapter.get("adapter_name", "")
    if adapter.get("graphics_backend", "").lower() == "gl" and name.startswith(
        "D3D12 ("
    ):
        if any(s in name.lower() for s in ("nvidia", "radeon", "amd")):
            return "discrete_gpu"
        if "intel" in name.lower():
            return "integrated_gpu"
    return {
        "discretegpu": "discrete_gpu",
        "integratedgpu": "integrated_gpu",
        "virtualgpu": "virtual_gpu",
        "cpu": "cpu",
    }.get(adapter.get("device_type", "").lower(), "unknown")


def render_vestra(definition, inputs, output, backend, *, progress=None):
    started = time.perf_counter()
    project = vestra_project(definition, inputs)
    built = time.perf_counter()
    project.snapshot(output=output).save(output.with_suffix(".project.json"))
    result = project.render(
        output,
        backend="wgpu" if backend == "hardware-wgpu" else "cpu",
        show_progress=False,
        on_progress=progress,
    )
    adapter = None
    if result.adapter:
        a = result.adapter
        adapter = {
            k: getattr(a, k)
            for k in (
                "adapter_name",
                "driver_name",
                "driver_info",
                "vendor_id",
                "device_id",
            )
        }
        adapter.update(
            device_type=a.device_type.value, graphics_backend=a.graphics_backend.value
        )
        adapter["performance_class"] = adapter_class(adapter)
    timings = {
        name: getattr(result.timings, name)
        for name in dir(result.timings)
        if name.endswith("_ms")
    }
    return dict(
        requested_render_backend="wgpu" if backend == "hardware-wgpu" else "cpu",
        requested_graphics_api=os.environ.get("VESTRA_WGPU_BACKEND"),
        render_backend=result.selected_backend,
        adapter=adapter,
        encoder_backend=result.encoder_backend,
        total_frames=result.total_frames,
        width=result.width,
        height=result.height,
        frame_rate=result.frame_rate,
        timing_scope=result.timing_scope.value,
        timings=timings,
        performance={
            k: getattr(result.performance, k)
            for k in dir(result.performance)
            if not k.startswith("_")
        },
        project_build_ms=(built - started) * 1000,
        fallback=result.fallback.message if result.fallback else None,
    )


def render_moviepy(definition, inputs, output, *, progress=None):
    # Use the same system FFmpeg executable as Vestra, not imageio's bundled one.
    import shutil

    os.environ["FFMPEG_BINARY"] = shutil.which("ffmpeg") or "ffmpeg"
    from moviepy import VideoFileClip, CompositeVideoClip, ImageClip, vfx
    import numpy as np

    started = time.perf_counter()
    sources, clips = [], []
    w, h = definition["width"], definition["height"]
    try:
        for c in definition["clips"]:
            source = VideoFileClip(str(inputs / f"{c['asset']}.mp4"), audio=False)
            sources.append(source)
            cw, ch = int(w * c["scale"]), int(h * c["scale"])
            offset = c.get("source_start", 0)
            clip = source.subclipped(offset, offset + c["duration"]).resized((cw, ch))
            if c["brightness"]:
                clip = clip.with_effects([vfx.LumContrast(lum=c["brightness"] * 255)])
            if c["ellipse"]:
                yy, xx = np.ogrid[:ch, :cw]
                mask = (
                    ((xx - cw / 2) / (cw * 0.45)) ** 2
                    + ((yy - ch / 2) / (ch * 0.45)) ** 2
                    <= 1
                ).astype(float)
                clip = clip.with_mask(ImageClip(mask, is_mask=True))

            def position(t, c=c, cw=cw, ch=ch):
                x = c["x"] + (c["end_x"] - c["x"]) * t / c["duration"]
                return (x * w - cw / 2, c["y"] * h - ch / 2)

            clip = clip.with_position(position)
            if c["opacity"] != 1:
                clip = clip.with_opacity(c["opacity"])
            if c["fade_in"]:
                clip = clip.with_effects([vfx.CrossFadeIn(c["fade_in"])])
            clips.append(clip.with_start(c["start"]))
        final = (
            clips[0]
            if len(clips) == 1
            else CompositeVideoClip(clips, size=(w, h), bg_color=(0, 0, 0))
        ).with_duration(definition["duration"])
        built = time.perf_counter()
        final.write_videofile(
            str(output),
            fps=30,
            codec="libx264",
            audio=False,
            preset="veryfast",
            ffmpeg_params=["-crf", "23", "-pix_fmt", "yuv420p"],
            logger=progress,
        )
        ended = time.perf_counter()
        final.close()
        return dict(
            render_backend="cpu",
            adapter=None,
            encoder_backend="libx264",
            timing_scope="one_shot",
            project_build_ms=(built - started) * 1000,
            timings={"write_videofile_ms": (ended - built) * 1000},
        )
    finally:
        for source in sources:
            source.close()
