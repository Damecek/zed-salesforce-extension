#!/usr/bin/env python3
"""Verify repository Python smoke tests load on the supported baseline."""

import importlib.util
import sys
import unittest
from pathlib import Path


SCRIPTS_DIR = Path(__file__).resolve().parent


def load_script(name):
    path = SCRIPTS_DIR / name
    spec = importlib.util.spec_from_file_location(path.stem, path)
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


class PythonBaselineTests(unittest.TestCase):
    def test_visualforce_integration_loads_on_python_3_10(self):
        self.assertGreaterEqual(sys.version_info, (3, 10))
        module = load_script("test-visualforce-integration.py")

        self.assertEqual(module.MINIMUM_PYTHON, (3, 10))

    def test_visualforce_integration_rejects_older_python(self):
        module = load_script("test-visualforce-integration.py")

        with self.assertRaisesRegex(RuntimeError, "Python 3.10 or newer"):
            module.require_supported_python((3, 9))


if __name__ == "__main__":
    unittest.main()
