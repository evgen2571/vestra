from pathlib import Path

import pytest

import vestra


FIXTURE = Path("tests/fixtures/wgpu-small-rgba.json")


def request(output: Path, **kwargs: object) -> vestra.RenderRequest:
    return vestra.RenderRequest(
        output, backend=vestra.BackendPreference.CPU, **kwargs
    )


def multi_frame_project(tmp_path: Path) -> vestra.ProjectSnapshot:
    return vestra.ProjectSnapshot.from_dict(
        {
            "schema_version": 2,
            "output": {
                "path": "unused.mp4", "width": 2, "height": 2, "frame_rate": "1/1",
                "background": "#102030", "quality": "preview", "audio": False,
                "duration_mode": "explicit", "duration": 3,
            },
            "assets": [], "visual": {"clips": []},
        },
        base_directory=tmp_path,
    )


def test_one_shot_callback_failure_on_started_preserves_original_error(tmp_path: Path) -> None:
    class OneShotStartedFailure(Exception):
        pass

    output = tmp_path / "started.mp4"
    seen: list[str] = []

    def callback(event: vestra.RenderEvent) -> None:
        seen.append(event.kind)
        raise OneShotStartedFailure("one-shot started failure")

    with pytest.raises(OneShotStartedFailure, match="one-shot started failure") as raised:
        vestra.Editor().render(vestra.ProjectSnapshot.load(FIXTURE), request(output), progress=callback)

    assert seen == ["started"]
    assert not output.exists()
    assert list(tmp_path.iterdir()) == []
    assert isinstance(raised.value.render_cleanup_error, vestra.CancelledError)


def test_one_shot_callback_failure_after_progress_stops_later_callbacks(tmp_path: Path) -> None:
    class OneShotProgressFailure(Exception):
        pass

    output = tmp_path / "progress.mp4"
    seen: list[str] = []

    def callback(event: vestra.RenderEvent) -> None:
        seen.append(event.kind)
        if event.kind == "progress":
            raise OneShotProgressFailure("one-shot progress failure")

    with pytest.raises(OneShotProgressFailure, match="one-shot progress failure") as raised:
        vestra.Editor().render(multi_frame_project(tmp_path), request(output), progress=callback)

    assert seen == ["started", "progress"]
    assert not output.exists()
    assert list(tmp_path.iterdir()) == []
    assert isinstance(raised.value.render_cleanup_error, vestra.CancelledError)


def test_one_shot_already_cancelled_and_progress_cancellation_clean_up(tmp_path: Path) -> None:
    token = vestra.CancellationToken()
    token.cancel()
    output = tmp_path / "already-cancelled.mp4"
    with pytest.raises(vestra.CancelledError) as raised:
        vestra.Editor().render(vestra.ProjectSnapshot.load(FIXTURE), request(output), cancellation=token)
    assert raised.value.temporary_removed is True
    assert token.is_cancelled
    assert not output.exists()
    assert list(tmp_path.iterdir()) == []

    progress_token = vestra.CancellationToken()
    progress_output = tmp_path / "progress-cancelled.mp4"
    seen: list[str] = []

    def cancel(event: vestra.RenderEvent) -> None:
        seen.append(event.kind)
        if event.kind == "progress":
            progress_token.cancel()

    with pytest.raises(vestra.CancelledError) as progress_error:
        vestra.Editor().render(
            multi_frame_project(tmp_path), request(progress_output),
            progress=cancel, cancellation=progress_token,
        )
    assert progress_error.value.temporary_removed is True
    assert progress_token.is_cancelled
    assert seen == ["started", "progress"]
    assert not progress_output.exists()
    assert list(tmp_path.iterdir()) == []


def test_one_shot_overwrite_and_event_policy(tmp_path: Path) -> None:
    output = tmp_path / "existing.mp4"
    original = b"keep this output"
    output.write_bytes(original)
    with pytest.raises(vestra.RenderError) as raised:
        vestra.Editor().render(vestra.ProjectSnapshot.load(FIXTURE), request(output))
    assert raised.value.kind == "project"
    assert output.read_bytes() == original
    assert list(tmp_path.iterdir()) == [output]

    events: list[vestra.RenderEvent] = []
    result = vestra.Editor().render(
        vestra.ProjectSnapshot.load(FIXTURE), request(output, overwrite=True), progress=events.append
    )
    assert result.output_path == output
    assert output.read_bytes() != original
    assert [event.kind for event in events] == ["started"]
    assert list(tmp_path.iterdir()) == [output]
