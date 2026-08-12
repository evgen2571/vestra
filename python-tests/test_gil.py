import threading


def test_python_executes_while_native_work_is_detached() -> None:
    import vestra._native as native

    native_wait_while_detached = getattr(native, "_test_wait_while_detached")
    native_wait_until_detached_entered = getattr(
        native, "_test_wait_until_detached_entered"
    )
    native_release_detached_wait = getattr(native, "_test_release_detached_wait")
    completed = threading.Event()
    worker_errors: list[BaseException] = []

    def wait_while_detached() -> None:
        try:
            native_wait_while_detached()
        except BaseException as error:
            worker_errors.append(error)
        else:
            completed.set()

    worker = threading.Thread(target=wait_while_detached)
    worker.start()

    try:
        native_wait_until_detached_entered()
        # The worker is now blocked after Python::detach. This Python work can
        # execute only because that native worker released the interpreter GIL.
        progress = sum(range(10))
    finally:
        native_release_detached_wait()
        worker.join(timeout=1)

    assert not worker.is_alive()
    if worker_errors:
        raise worker_errors[0]
    assert completed.is_set()
    assert progress == 45
