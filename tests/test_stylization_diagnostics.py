"""The diagnostic harness orders timestamps numerically and rejects incomplete pairs."""

import importlib.util
from pathlib import Path
from tempfile import TemporaryDirectory
import unittest

SPEC = importlib.util.spec_from_file_location(
    "stylization_diagnostics",
    Path(__file__).resolve().parents[1] / "scripts/stylization-diagnostics.py",
)
DIAGNOSTICS = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(DIAGNOSTICS)


class DiagnosticsTests(unittest.TestCase):
    def test_numeric_frame_order_and_missing_counterpart(self):
        with TemporaryDirectory() as temporary:
            directory = Path(temporary)
            for time in [10, 2, 100]:
                (directory / f"cpu-{time}.png").touch()
                (directory / f"wgpu-{time}.png").touch()
            self.assertEqual(
                [pair[0] for pair in DIAGNOSTICS.frame_pairs(directory)], [2, 10, 100]
            )
            (directory / "wgpu-2.png").unlink()
            with self.assertRaisesRegex(ValueError, "missing WGPU counterpart"):
                DIAGNOSTICS.frame_pairs(directory)


if __name__ == "__main__":
    unittest.main()
