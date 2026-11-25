import os
from pathlib import Path

import pytest

from vestra import AdapterDeviceType, PreparationError, Project, SubtitleCue, Subtitles


def test_srt_import_keeps_multiline_text_and_orders_cues(tmp_path: Path) -> None:
    path = tmp_path / "captions.srt"
    path.write_bytes(
        "\ufeff12\r\n00:00:02,500 --> 00:00:03,750\r\nTwo\r\nlines!\r\n"
        " \r\n3\r\n00:00:00,250 --> 00:00:01,000\r\nOne line…".encode()
    )
    subtitles = Subtitles.from_srt(path)

    assert subtitles.cues == (
        SubtitleCue(0.25, 1.0, "One line…"),
        SubtitleCue(2.5, 3.75, "Two\nlines!"),
    )
    assert subtitles.duration == 3.75
    placed = subtitles.to_group(font_size=24, max_width=140, fill="#12caab")

    project = Project(size=(160, 100), fps=24)
    project.root.add(placed, duration=subtitles.duration)
    source = project.snapshot().to_dict()["visual"]["clips"][0]["source"]
    assert source["type"] == "group"
    assert [
        (clip["start"], clip["duration"], clip["source"]["text"])
        for clip in source["clips"]
    ] == [(0.25, 0.75, "One line…"), (2.5, 1.25, "Two\nlines!")]
    for clip in source["clips"]:
        assert clip["source"]["type"] == "text"
        assert clip["source"]["align"] == "center"
        assert clip["source"]["font_size"] == 24
        assert clip["source"]["max_width"] == 140
        assert clip["source"]["fill"] == "#12caab"


@pytest.mark.parametrize(
    "contents,error",
    [
        ("seven\n00:00:00,000 --> 00:00:01,000\nLine", "cue block 1"),
        ("7\n00:60:00,000 --> 00:00:01,000\nLine", "cue 7.*timing"),
        ("7\n00:00:00.000 --> 00:00:01.000\nLine", "cue 7.*timing"),
        ("7\n00:00:00,000 --> 00:00:01,000\n   ", "cue 7.*text"),
        ("7\n00:00:01,000 --> 00:00:00,000\nLine", "cue 7.*end"),
    ],
)
def test_malformed_srt_reports_cue_context(
    tmp_path: Path, contents: str, error: str
) -> None:
    path = tmp_path / "invalid.srt"
    path.write_text(contents, encoding="utf-8")
    with pytest.raises(ValueError, match=error):
        Subtitles.from_srt(path)


@pytest.mark.parametrize(
    "cue,error",
    [
        ((-1, 0, "Line"), "start"),
        ((0, float("inf"), "Line"), "end"),
        ((float("nan"), 1, "Line"), "start"),
        ((0, 0.1e-9, "Line"), "end"),
        ((0, 10**1000, "Line"), "end"),
        ((False, 1, "Line"), "start"),
        ((0, 1, "  "), "text"),
        ((0, 1, 4), "text"),
        ((0, 1), "tuple"),
    ],
)
def test_explicit_cues_validate_timing_and_text(cue: tuple, error: str) -> None:
    with pytest.raises((TypeError, ValueError), match=f"subtitle cue 1.*{error}"):
        Subtitles([cue])


def test_overlapping_cues_are_rejected_and_empty_tracks_are_allowed() -> None:
    with pytest.raises(ValueError, match="cue 1 overlaps cue 2"):
        Subtitles([(1.5, 3, "Second"), (1, 2, "First")])
    empty = Subtitles([])
    assert empty.cues == ()
    assert empty.duration == 0
    assert empty.to_group().children == ()


def test_fractional_nanosecond_endpoints_share_one_adjacent_boundary() -> None:
    subtitles = Subtitles(
        [(0.00100000051, 0.03336666649, "First"), (0.03336666649, 0.1, "Second")]
    )
    first, second = subtitles.to_group().children
    assert first.duration == 0.032366665
    assert second.start == 0.03336666649


def _nested_caption_project(*, styled: bool) -> Project:
    subtitles = Subtitles(
        [
            (0.033366667, 0.066733334, "ONE"),
            (0.066733334, 0.1001, "TWO\nONE"),
            (0.133466667, 0.166833334, "THREE"),
        ]
    )
    project = Project(
        size=(160, 100), fps=(30_000, 1001), duration=0.4, background="#102040"
    )
    parent = project.root.group(start=0.1001, duration=0.25)
    parent.add(
        subtitles.to_group(
            font=Path("tests/assets/VestraTest-Regular.ttf"),
            font_size=20,
            max_width=120,
            stroke_color="#20e02080" if styled else None,
            stroke_width=3 if styled else 0,
            background_color="#d0202080" if styled else None,
            background_padding=4 if styled else 0,
        ),
        start=0.033366666,
        duration=subtitles.duration,
    )
    return project


@pytest.mark.parametrize("styled", [False, True])
def test_nested_caption_boundaries_and_bottom_alignment_on_fractional_grid(
    styled: bool,
) -> None:
    project = _nested_caption_project(styled=styled)
    prepared = project.prepare(backend="cpu")

    def frame(number: int) -> bytes:
        return prepared.render_frame_number(number).to_bytes()

    backdrop = frame(0)
    assert backdrop == bytes((16, 32, 64, 255)) * (160 * 100)
    for number in (4, 7, 9):
        assert frame(number) == backdrop
    one, two, three = [frame(number) for number in (5, 6, 8)]
    assert all(pixels != backdrop for pixels in (one, two, three))
    assert one != three

    def ink_rows(pixels: bytes) -> list[int]:
        stride = 160 * 4
        return [
            y
            for y in range(100)
            if pixels[y * stride : (y + 1) * stride]
            != backdrop[y * stride : (y + 1) * stride]
        ]

    ys_one = ink_rows(one)
    ys_two = ink_rows(two)
    assert min(ys_two) < min(ys_one)
    assert max(ys_two) == max(ys_one)


def test_caption_style_composites_alpha_without_moving_glyphs() -> None:
    unstyled = _nested_caption_project(styled=False).prepare(backend="cpu")
    styled = _nested_caption_project(styled=True).prepare(backend="cpu")
    plain = unstyled.render_frame_number(5).to_bytes()
    decorated = styled.render_frame_number(5).to_bytes()
    box_pixel = (92 * 160 + 20) * 4
    assert decorated[box_pixel : box_pixel + 4] == bytes((112, 32, 48, 255))
    decorated_pixels = [decorated[i : i + 4] for i in range(0, len(decorated), 4)]
    assert bytes((72, 128, 40, 255)) in decorated_pixels
    white = bytes((255, 255, 255, 255))
    plain_glyphs = [i for i in range(0, len(plain), 4) if plain[i : i + 4] == white]
    styled_glyphs = [
        i * 4 for i, pixel in enumerate(decorated_pixels) if pixel == white
    ]
    assert plain_glyphs
    assert styled_glyphs == plain_glyphs


def test_styled_caption_matches_wgpu_across_gaps_and_adjacent_cues() -> None:
    project = _nested_caption_project(styled=True)
    cpu = project.prepare(backend="cpu")
    try:
        wgpu = project.prepare(backend="wgpu")
    except PreparationError:
        if os.environ.get("VESTRA_REQUIRE_WGPU") == "1":
            raise
        pytest.skip("no compatible WGPU adapter")
    if os.environ.get("VESTRA_REQUIRE_HARDWARE_WGPU") == "1":
        adapter = wgpu.preparation_report.adapter
        assert adapter is not None
        assert adapter.device_type in (
            AdapterDeviceType.DISCRETE_GPU,
            AdapterDeviceType.INTEGRATED_GPU,
        )
    for number in (8, 5, 9, 6, 4, 7, 5):
        expected = cpu.render_frame_number(number).to_bytes()
        actual = wgpu.render_frame_number(number).to_bytes()
        assert max(abs(a - b) for a, b in zip(expected, actual, strict=True)) <= 2
