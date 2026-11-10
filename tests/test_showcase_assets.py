"""Asset preparation must verify bundled inputs and create deterministic bounded audio."""

import importlib.util
from pathlib import Path
import wave


SPEC = importlib.util.spec_from_file_location(
    "showcase_assets",
    Path(__file__).resolve().parents[1] / "examples/showcase/prepare-assets.py",
)
assets = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(assets)


def test_bundled_footage_is_shared_and_checksum_verified() -> None:
    manifest = assets.load_inputs(assets.ROOT.parents[1] / "assets/footage")
    assert set(manifest["prepared"]) == {"forest", "valley", "tear"}
    assert manifest["settings"]["duration"] == 12
    assert manifest["settings"]["width"] == 1920
    assert all(
        item["validation"]["full_decode"] for item in manifest["prepared"].values()
    )


def test_synth_audio_is_deterministic_and_has_the_documented_duration(
    tmp_path: Path,
) -> None:
    first, second = tmp_path / "one.wav", tmp_path / "two.wav"
    assets.write_audio(first)
    assets.write_audio(second)
    assert first.read_bytes() == second.read_bytes()
    with wave.open(str(first)) as recording:
        assert recording.getframerate() == 48000
        assert recording.getnchannels() == 1
        assert recording.getnframes() == 16 * 48000
        assert recording.getsampwidth() == 2
