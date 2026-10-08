"""Executable smoke and boundary tests for the visual-regression skill tools."""

import json
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest

from PIL import Image


SCRIPTS = Path(__file__).resolve().parents[1] / "scripts"


class VisualToolsTest(unittest.TestCase):
    def setUp(self):
        self.folder = tempfile.TemporaryDirectory()
        self.addCleanup(self.folder.cleanup)
        self.root = Path(self.folder.name)
        self.reference = self.root / "reference.png"
        self.candidate = self.root / "candidate.png"
        Image.new("RGBA", (3, 2), (10, 20, 30, 255)).save(self.reference)
        Image.new("RGBA", (3, 2), (10, 20, 30, 255)).save(self.candidate)

    def run_tool(self, script, *args):
        return subprocess.run(
            [sys.executable, str(SCRIPTS / script), *map(str, args)],
            capture_output=True,
            text=True,
            check=False,
        )

    def test_identical_images(self):
        result = self.run_tool(
            "compare_frames.py", self.reference, self.candidate,
            "--output-dir", self.root / "diff", "--max-mae", "0",
            "--max-mismatch-fraction", "0",
        )
        self.assertEqual(result.returncode, 0, result.stderr)
        metrics = json.loads((self.root / "diff/metrics.json").read_text())
        self.assertEqual(metrics["mae"], 0)
        self.assertEqual(metrics["mismatch_fraction"], 0)
        self.assertIsNone(metrics["psnr_db"])
        self.assertTrue((self.root / "diff/difference.png").exists())

    def test_difference_and_threshold_failure(self):
        image = Image.open(self.candidate)
        image.putpixel((0, 0), (60, 20, 30, 255))
        image.save(self.candidate)
        result = self.run_tool(
            "compare_frames.py", self.reference, self.candidate,
            "--output-dir", self.root / "diff", "--max-mae", "0",
        )
        self.assertEqual(result.returncode, 1, result.stderr)
        metrics = json.loads((self.root / "diff/metrics.json").read_text())
        self.assertGreater(metrics["mae"], 0)
        self.assertAlmostEqual(metrics["mismatch_fraction"], 1 / 6, places=7)
        tolerated = self.run_tool(
            "compare_frames.py", self.reference, self.candidate,
            "--output-dir", self.root / "tolerated", "--pixel-tolerance", "50",
            "--max-mismatch-fraction", "0",
        )
        self.assertEqual(tolerated.returncode, 0, tolerated.stderr)

    def test_rejects_different_dimensions(self):
        Image.new("RGB", (4, 2)).save(self.candidate)
        result = self.run_tool(
            "compare_frames.py", self.reference, self.candidate,
            "--output-dir", self.root / "diff",
        )
        self.assertEqual(result.returncode, 2)
        self.assertIn("frame sizes differ", result.stderr)

    def test_contact_sheet_and_label_validation(self):
        output = self.root / "sheet.png"
        result = self.run_tool(
            "create_contact_sheet.py", self.reference, self.candidate,
            "--labels", "cpu", "wgpu", "--columns", "2", "--tile-width", "50",
            "--tile-height", "30", "--output", output,
        )
        self.assertEqual(result.returncode, 0, result.stderr)
        with Image.open(output) as image:
            self.assertEqual(image.size, (136, 86))
        invalid = self.run_tool(
            "create_contact_sheet.py", self.reference, self.candidate,
            "--labels", "only-one", "--output", output,
        )
        self.assertEqual(invalid.returncode, 2)


if __name__ == "__main__":
    unittest.main()
