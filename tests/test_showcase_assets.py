"""Asset preparation must verify downloads and create deterministic bounded audio."""

import importlib.util
from pathlib import Path
import hashlib
import wave

import pytest

SPEC = importlib.util.spec_from_file_location(
    "showcase_assets",
    Path(__file__).resolve().parents[1] / "examples/showcase/prepare-assets.py",
)
assets = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(assets)


def test_verified_download_reuses_cache_and_rejects_wrong_content(
    tmp_path: Path,
) -> None:
    source = tmp_path / "source.bin"
    source.write_bytes(b"licensed fixture")
    target = tmp_path / "cache.bin"
    digest = hashlib.sha256(b"licensed fixture").hexdigest()
    assets.download(source.as_uri(), digest, target)
    assert target.read_bytes() == b"licensed fixture"
    source.unlink()
    assets.download(source.as_uri(), digest, target)
    target.write_bytes(b"corrupt")
    source.write_bytes(b"wrong source")
    with pytest.raises(ValueError, match="checksum"):
        assets.download(source.as_uri(), digest, target)
    assert target.read_bytes() == b"corrupt", (
        "unverified downloads must never replace the cache"
    )
    assert not list(tmp_path.glob("*.partial"))


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
