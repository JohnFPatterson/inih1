#!/usr/bin/env python3
"""Print the public function names declared in ini.h, one per line."""

from __future__ import annotations

import re
import sys
from pathlib import Path

NAMES = (
    "ini_parse",
    "ini_parse_file",
    "ini_parse_stream",
    "ini_parse_string",
    "ini_parse_string_length",
)


def main() -> int:
    root = Path(__file__).resolve().parents[1]
    text = (root / "ini.h").read_text(encoding="utf-8")
    for name in NAMES:
        hits = re.findall(rf"\bint {name}\(", text)
        if len(hits) != 1:
            print(f"{name}: expected 1 declaration, found {len(hits)}", file=sys.stderr)
            return 1
        print(name)
    return 0


if __name__ == "__main__":
    sys.exit(main())
