from pathlib import Path
import threading

import pytest

import video_editor


def prepared() -> video_editor.PreparedProject:
    return video_editor.Editor().prepare(
        video_editor.Project.load(Path("tests/fixtures/wgpu-small-rgba.json")),
        video_editor.PrepareOptions(backend=video_editor.BackendPreference.CPU),
    )


def test_prepared_frame_operation_releases_gil_and_rejects_same_object_busy() -> None:
    import video_editor._native as native

    value = prepared()
    native._test_arm_prepared_operation()
    completed = threading.Event()
    errors: list[BaseException] = []

    def render() -> None:
        try:
            value.render_frame_number(0)
        except BaseException as error:
            errors.append(error)
        else:
            completed.set()

    worker = threading.Thread(target=render)
    worker.start()
    try:
        native._test_wait_until_prepared_operation_entered()
        assert sum(range(10)) == 45
        with pytest.raises(video_editor.PreparedProjectBusyError) as raised:
            value.render_frame_number(0)
        assert raised.value.kind == "busy"
        assert raised.value.diagnostics == ()
        assert raised.value.warnings == ()
    finally:
        native._test_release_prepared_operation()
        worker.join(timeout=1)
    assert not worker.is_alive()
    assert not errors
    assert completed.is_set()
    assert value.render_frame_number(0).frame_number == 0


def test_preparation_releases_gil() -> None:
    import video_editor._native as native

    project = video_editor.Project.load(Path("tests/fixtures/wgpu-small-rgba.json"))
    native._test_arm_prepared_operation()
    completed = threading.Event()
    errors: list[BaseException] = []

    def prepare_in_thread() -> None:
        try:
            video_editor.Editor().prepare(
                project,
                video_editor.PrepareOptions(backend=video_editor.BackendPreference.CPU),
            )
        except BaseException as error:
            errors.append(error)
        else:
            completed.set()

    worker = threading.Thread(target=prepare_in_thread)
    worker.start()
    try:
        native._test_wait_until_prepared_operation_entered()
        assert sum(range(10)) == 45
    finally:
        native._test_release_prepared_operation()
        worker.join(timeout=1)
    assert not worker.is_alive()
    assert not errors
    assert completed.is_set()


def test_busy_state_is_local_to_one_prepared_object() -> None:
    import video_editor._native as native

    first, second = prepared(), prepared()
    native._test_arm_prepared_operation()
    worker_errors: list[BaseException] = []

    def render_first() -> None:
        try:
            first.render_frame_number(0)
        except BaseException as error:
            worker_errors.append(error)

    worker = threading.Thread(target=render_first)
    worker.start()
    try:
        native._test_wait_until_prepared_operation_entered()
        assert second.render_frame_number(0).frame_number == 0
    finally:
        native._test_release_prepared_operation()
        worker.join(timeout=1)
    assert not worker.is_alive()
    assert worker_errors == []


def _assert_busy(call: object) -> None:
    with pytest.raises(video_editor.PreparedProjectBusyError) as raised:
        call()  # type: ignore[operator]
    assert raised.value.kind == "busy"
    assert raised.value.diagnostics == ()
    assert raised.value.warnings == ()


def test_video_operation_rejects_video_and_frame_calls_while_active(tmp_path: Path) -> None:
    import video_editor._native as native

    value = prepared()
    output = tmp_path / "active.mp4"
    errors: list[BaseException] = []
    completed = threading.Event()
    native._test_arm_video_render()

    def render() -> None:
        try:
            value.render_video(video_editor.PreparedVideoRenderRequest(output))
        except BaseException as error:
            errors.append(error)
        else:
            completed.set()

    worker = threading.Thread(target=render)
    worker.start()
    try:
        native._test_wait_until_video_render_entered()
        _assert_busy(lambda: value.render_video(video_editor.PreparedVideoRenderRequest(tmp_path / "busy.mp4")))
        _assert_busy(lambda: value.render_frame_number(0))
    finally:
        native._test_release_video_render()
        worker.join()

    assert not worker.is_alive()
    assert errors == []
    assert completed.is_set()
    assert value.render_frame_number(0).frame_number == 0


def test_frame_operation_rejects_video_call_while_active(tmp_path: Path) -> None:
    import video_editor._native as native

    value = prepared()
    errors: list[BaseException] = []
    native._test_arm_prepared_operation()

    def render_frame() -> None:
        try:
            value.render_frame_number(0)
        except BaseException as error:
            errors.append(error)

    worker = threading.Thread(target=render_frame)
    worker.start()
    try:
        native._test_wait_until_prepared_operation_entered()
        _assert_busy(lambda: value.render_video(video_editor.PreparedVideoRenderRequest(tmp_path / "busy.mp4")))
    finally:
        native._test_release_prepared_operation()
        worker.join()

    assert not worker.is_alive()
    assert errors == []
    assert value.render_frame_number(0).frame_number == 0
