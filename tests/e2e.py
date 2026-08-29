#!/usr/bin/env python3
"""Entry point for the end-to-end suite. Run it via tests/sandboxed.sh."""

import sys
import os

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))

from harness import main  # noqa: E402
import cases  # noqa: E402,F401

if __name__ == "__main__":
    code = main(sys.argv[1:])
    sys.stdout.flush()
    os._exit(code)
