from pathlib import Path
import subprocess
import threading

import pytest

import vestra


FIXTURE = Path("tests/fixtures/wgpu-small-rgba.json")


def cpu_prepared() -> vestra.PreparedProject:
    return vestra.Editor().prepare(
        vestra.Project.load(FIXTURE),
        vestra.PrepareOptions(backend=vestra.BackendPreference.CPU),
    )


def multi_frame_prepared(tmp_path: Path) -> vestra.PreparedProject:
    project = vestra.Project.from_dict(
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
    return vestra.Editor().prepare(
        project, vestra.PrepareOptions(backend=vestra.BackendPreference.CPU)
    )


def test_prepared_video_render_publishes_output_and_filters_completed(tmp_path: Path) -> None:
    output = tmp_path / "prepared.mp4"
    events: list[vestra.RenderEvent] = []
    result = cpu_prepared().render_video(
        vestra.PreparedVideoRenderRequest(output), progress=events.append
    )

    assert output.exists() and output.stat().st_size > 0
    assert result.output_path == output
    assert result.timing_scope is vestra.RenderTimingScope.PREPARED_OPERATION
    assert result.performance.rendered_frame_count == result.total_frames
    assert [event.kind for event in events] == ["started"]
    assert events[0].output_path == output
    assert events[0].progress == 0.0
    subprocess.run(["ffprobe", "-v", "error", "-show_format", str(output)], check=True)


def test_prepared_render_without_callback_never_reattaches_to_python(tmp_path: Path) -> None:
    import vestra._native as native

    output = tmp_path / "prepared-no-callback.mp4"
    native._test_reset_callback_attach_count()

    result = cpu_prepared().render_video(vestra.PreparedVideoRenderRequest(output))

    assert result.output_path == output
    assert output.exists()
    assert native._test_callback_attach_count() == 0


def test_one_shot_video_render_uses_one_shot_timing_scope(tmp_path: Path) -> None:
    output = tmp_path / "one-shot.mp4"
    result = vestra.Editor().render(
        vestra.Project.load(FIXTURE),
        vestra.RenderRequest(output, backend=vestra.BackendPreference.CPU),
    )
    assert output.exists() and output.stat().st_size > 0
    assert result.timing_scope is vestra.RenderTimingScope.ONE_SHOT
    assert result.selected_backend == "cpu"
    assert result.encoder_backend == "ffmpeg"


def test_one_shot_render_without_callback_never_reattaches_to_python(tmp_path: Path) -> None:
    import vestra._native as native

    output = tmp_path / "one-shot-no-callback.mp4"
    native._test_reset_callback_attach_count()

    result = vestra.Editor().render(
        vestra.Project.load(FIXTURE),
        vestra.RenderRequest(output, backend=vestra.BackendPreference.CPU),
    )

    assert result.output_path == output
    assert output.exists()
    assert native._test_callback_attach_count() == 0


def test_callback_attachment_count_matches_forwarded_events(tmp_path: Path) -> None:
    import vestra._native as native

    prepared_events: list[vestra.RenderEvent] = []
    native._test_reset_callback_attach_count()
    multi_frame_prepared(tmp_path).render_video(
        vestra.PreparedVideoRenderRequest(tmp_path / "prepared-callback-count.mp4"),
        progress=prepared_events.append,
    )
    assert prepared_events
    assert native._test_callback_attach_count() == len(prepared_events)

    one_shot_events: list[vestra.RenderEvent] = []
    native._test_reset_callback_attach_count()
    vestra.Editor().render(
        vestra.Project.load(FIXTURE),
        vestra.RenderRequest(
            tmp_path / "one-shot-callback-count.mp4",
            backend=vestra.BackendPreference.CPU,
        ),
        progress=one_shot_events.append,
    )
    assert one_shot_events
    assert native._test_callback_attach_count() == len(one_shot_events)


def test_cancelled_token_is_structured_and_prepared_object_is_reusable(tmp_path: Path) -> None:
    prepared = cpu_prepared()
    token = vestra.CancellationToken()
    token.cancel()
    output = tmp_path / "cancelled.mp4"
    with pytest.raises(vestra.CancelledError) as raised:
        prepared.render_video(vestra.PreparedVideoRenderRequest(output), cancellation=token)
    assert raised.value.kind == "render"
    assert raised.value.failure_context is not None
    assert raised.value.temporary_removed is True
    assert not output.exists()
    assert prepared.render_frame_number(0).width == 174


def test_callback_exception_is_preserved_and_suppresses_publication(tmp_path: Path) -> None:
    prepared = cpu_prepared()
    output = tmp_path / "callback.mp4"

    class CallbackFailure(Exception):
        pass

    def fail(_: vestra.RenderEvent) -> None:
        raise CallbackFailure("failed immediately")

    with pytest.raises(CallbackFailure, match="failed immediately"):
        prepared.render_video(vestra.PreparedVideoRenderRequest(output), progress=fail)
    assert not output.exists()
    assert prepared.render_frame_number(0).frame_number == 0


def test_callback_reentrancy_is_immediately_busy_and_outer_render_continues(tmp_path: Path) -> None:
    prepared = cpu_prepared()
    output = tmp_path / "reentrant.mp4"
    busy: list[bool] = []

    def callback(_: vestra.RenderEvent) -> None:
        with pytest.raises(vestra.PreparedProjectBusyError):
            prepared.render_frame_number(0)
        with pytest.raises(vestra.PreparedProjectBusyError):
            prepared.render_video(vestra.PreparedVideoRenderRequest(tmp_path / "inner.mp4"))
        busy.append(True)

    prepared.render_video(vestra.PreparedVideoRenderRequest(output), progress=callback)
    assert busy == [True]
    assert output.exists()


def test_callback_failure_after_progress_invalidates_the_native_prepared_state(tmp_path: Path) -> None:
    import vestra._native as native

    prepared = multi_frame_prepared(tmp_path)
    output = tmp_path / "after-progress.mp4"
    seen: list[str] = []

    def fail_on_progress(event: vestra.RenderEvent) -> None:
        seen.append(event.kind)
        if event.kind == "progress":
            raise RuntimeError("after progress")

    native._test_reset_callback_attach_count()
    with pytest.raises(RuntimeError, match="after progress") as raised:
        prepared.render_video(vestra.PreparedVideoRenderRequest(output), progress=fail_on_progress)
    assert seen == ["started", "progress"]
    assert native._test_callback_attach_count() == len(seen)
    assert not output.exists()
    cleanup_error = raised.value.render_cleanup_error
    assert isinstance(cleanup_error, vestra.CancelledError)
    assert cleanup_error.temporary_removed is True
    with pytest.raises(vestra.FrameRenderError) as raised:
        prepared.render_frame_number(0)
    assert raised.value.diagnostics[0].code == "MVP-PREPARED-INVALIDATED"


def test_existing_output_is_preserved_without_overwrite_and_replaced_with_it(tmp_path: Path) -> None:
    output = tmp_path / "existing.mp4"
    original = b"unchanged existing output"
    output.write_bytes(original)
    prepared = cpu_prepared()

    with pytest.raises(vestra.RenderError) as raised:
        prepared.render_video(vestra.PreparedVideoRenderRequest(output))
    assert raised.value.failure_context is not None
    assert output.read_bytes() == original
    assert list(tmp_path.iterdir()) == [output]
    assert prepared.render_frame_number(0).frame_number == 0

    result = prepared.render_video(
        vestra.PreparedVideoRenderRequest(output, overwrite=True)
    )
    assert result.output_path == output
    assert output.read_bytes() != original
    assert output.stat().st_size > 0
    assert list(tmp_path.iterdir()) == [output]


def test_callback_cancellation_removes_output_and_invalidates_after_submission(tmp_path: Path) -> None:
    prepared = multi_frame_prepared(tmp_path)
    token = vestra.CancellationToken()
    output = tmp_path / "callback-cancelled.mp4"
    events: list[str] = []

    def cancel_on_progress(event: vestra.RenderEvent) -> None:
        events.append(event.kind)
        if event.kind == "progress":
            token.cancel()

    with pytest.raises(vestra.CancelledError) as raised:
        prepared.render_video(
            vestra.PreparedVideoRenderRequest(output),
            progress=cancel_on_progress,
            cancellation=token,
        )
    assert raised.value.temporary_removed is True
    assert token.is_cancelled
    assert events == ["started", "progress"]
    assert not output.exists()
    assert list(tmp_path.iterdir()) == []
    with pytest.raises(vestra.FrameRenderError) as invalidated:
        prepared.render_frame_number(0)
    assert invalidated.value.diagnostics[0].code == "MVP-PREPARED-INVALIDATED"


def test_another_python_thread_can_cancel_while_the_callback_is_active(tmp_path: Path) -> None:
    prepared = cpu_prepared()
    token = vestra.CancellationToken()
    output = tmp_path / "thread-cancelled.mp4"
    entered = threading.Event()
    cancelled = threading.Event()

    def canceller() -> None:
        assert entered.wait(timeout=5)
        token.cancel()
        cancelled.set()

    thread = threading.Thread(target=canceller)
    thread.start()

    def wait_for_cancellation(_: vestra.RenderEvent) -> None:
        entered.set()
        assert cancelled.wait(timeout=5)

    try:
        with pytest.raises(vestra.CancelledError):
            prepared.render_video(
                vestra.PreparedVideoRenderRequest(output),
                progress=wait_for_cancellation,
                cancellation=token,
            )
    finally:
        thread.join(timeout=5)
    assert not thread.is_alive()
    assert token.is_cancelled
    assert not output.exists()


def test_non_callable_progress_is_rejected_before_prepared_work(tmp_path: Path) -> None:
    import vestra._native as native

    prepared = cpu_prepared()
    output = tmp_path / "invalid-progress.mp4"
    native._test_reset_native_render_invocation_count()

    with pytest.raises(TypeError, match="progress must be callable or None"):
        prepared.render_video(vestra.PreparedVideoRenderRequest(output), progress=object())

    assert not output.exists()
    assert list(tmp_path.iterdir()) == []
    assert native._test_native_render_invocation_count() == 0
    assert prepared.render_frame_number(0).frame_number == 0


def test_non_callable_progress_is_rejected_before_one_shot_preparation(tmp_path: Path) -> None:
    import vestra._native as native

    output = tmp_path / "invalid-one-shot-progress.mp4"
    events: list[vestra.RenderEvent] = []
    native._test_reset_native_render_invocation_count()

    with pytest.raises(TypeError, match="progress must be callable or None"):
        vestra.Editor().render(
            vestra.Project.load(FIXTURE),
            vestra.RenderRequest(output, backend=vestra.BackendPreference.CPU),
            progress=123,
        )

    assert events == []
    assert not output.exists()
    assert list(tmp_path.iterdir()) == []
    assert native._test_native_render_invocation_count() == 0


@pytest.mark.parametrize("value", [False, "cancel", 0, object()])
def test_callback_return_values_are_ignored(tmp_path: Path, value: object) -> None:
    output = tmp_path / "ignored-return.mp4"

    result = cpu_prepared().render_video(
        vestra.PreparedVideoRenderRequest(output), progress=lambda _: value
    )

    assert result.output_path == output
    assert output.exists()


def test_uncaught_reentrant_busy_error_is_the_callback_error(tmp_path: Path) -> None:
    prepared = cpu_prepared()
    output = tmp_path / "uncaught-reentrant.mp4"
    events: list[str] = []

    def callback(event: vestra.RenderEvent) -> None:
        events.append(event.kind)
        prepared.render_frame_number(0)

    with pytest.raises(vestra.PreparedProjectBusyError) as raised:
        prepared.render_video(vestra.PreparedVideoRenderRequest(output), progress=callback)

    assert raised.value.kind == "busy"
    assert events == ["started"]
    assert not output.exists()
    assert list(tmp_path.iterdir()) == []
    assert prepared.render_frame_number(0).frame_number == 0


def test_callbacks_run_on_the_calling_python_thread(tmp_path: Path) -> None:
    output = tmp_path / "thread-identity.mp4"
    caller = threading.get_ident()
    observed: list[int] = []

    cpu_prepared().render_video(
        vestra.PreparedVideoRenderRequest(output),
        progress=lambda _: observed.append(threading.get_ident()),
    )
    assert observed and observed == [caller] * len(observed)

    worker_output = tmp_path / "worker-thread-identity.mp4"
    callback_thread_ids: list[int] = []
    worker_thread_ids: list[int] = []
    worker_errors: list[BaseException] = []

    def render_from_worker() -> None:
        worker_thread_ids.append(threading.get_ident())
        try:
            cpu_prepared().render_video(
                vestra.PreparedVideoRenderRequest(worker_output),
                progress=lambda _: callback_thread_ids.append(threading.get_ident()),
            )
        except BaseException as error:
            worker_errors.append(error)

    thread = threading.Thread(target=render_from_worker)
    thread.start()
    thread.join(timeout=5)
    assert not thread.is_alive()
    assert worker_errors == []
    assert len(worker_thread_ids) == 1
    assert callback_thread_ids
    assert callback_thread_ids == [worker_thread_ids[0]] * len(callback_thread_ids)


def test_render_event_snapshots_keep_the_sdk_contract(tmp_path: Path) -> None:
    events: list[vestra.RenderEvent] = []
    output = tmp_path / "event-contract.mp4"
    multi_frame_prepared(tmp_path).render_video(
        vestra.PreparedVideoRenderRequest(output), progress=events.append
    )

    assert [event.kind for event in events] == ["started", "progress", "progress"]
    started, *progress = events
    assert started.schema_version == 1
    assert started.frame == 0
    assert started.total_frames == 3
    assert started.progress == 0.0
    assert started.output_path == output
    assert started.warnings == ()
    assert all(event.schema_version == 1 for event in events)
    assert [event.frame for event in progress] == [1, 2]
    assert all(event.total_frames == 3 for event in progress)
    assert all(event.progress is not None and event.progress < 1.0 for event in progress)
    assert all(event.output_path is None and event.warnings == () for event in progress)
    with pytest.raises(AttributeError):
        events[0].progress = 1.0  # type: ignore[misc]
    assert events[0].kind == "started"


def test_detached_native_render_observes_external_cancellation(tmp_path: Path) -> None:
    import vestra._native as native

    prepared = multi_frame_prepared(tmp_path)
    token = vestra.CancellationToken()
    output = tmp_path / "detached-cancelled.mp4"
    errors: list[BaseException] = []
    native._test_arm_video_render()

    def render() -> None:
        try:
            prepared.render_video(
                vestra.PreparedVideoRenderRequest(output), cancellation=token
            )
        except BaseException as error:
            errors.append(error)

    worker = threading.Thread(target=render)
    worker.start()
    try:
        native._test_wait_until_video_render_entered()
        token.cancel()
        assert token.is_cancelled
    finally:
        native._test_release_video_render()
        worker.join()

    assert not worker.is_alive()
    assert len(errors) == 1
    assert isinstance(errors[0], vestra.CancelledError)
    assert not output.exists()
    assert list(tmp_path.iterdir()) == []
    assert prepared.render_frame_number(0).frame_number == 0
