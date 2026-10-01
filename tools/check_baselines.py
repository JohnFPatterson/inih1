#!/usr/bin/env python3
"""Rebuild tests/baseline_*.txt from the C oracle and diff them.

The unittest binaries print a dumper transcript. The oracle prints a SAX trace.
This script applies the unittest.c / unittest_string.c / unittest_alloc.c dumper
rules to that trace and checks the bytes against the checked-in baselines.
"""

from __future__ import annotations

import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
ORACLE = ROOT / "build" / "oracle"

FILES = [
    "no_file.ini",
    "normal.ini",
    "bad_section.ini",
    "bad_comment.ini",
    "user_error.ini",
    "multi_line.ini",
    "bad_multi.ini",
    "bom.ini",
    "duplicate_sections.ini",
    "no_value.ini",
    "long_section.ini",
    "long_line.ini",
    "name_only_after_error.ini",
]

FILE_CONFIGS = [
    "multi",
    "multi_max_line",
    "single",
    "disallow_inline_comments",
    "stop_on_first_error",
    "handler_lineno",
    "heap",
    "heap_max_line",
    "heap_realloc",
    "heap_realloc_max_line",
    "call_handler_on_new_section",
    "allow_no_value",
]

STRING_CASES = [
    "empty string",
    "basic",
    "crlf",
    "long line",
    "long continued",
    "error",
]


def unescape(field: str) -> str | None:
    if field == r"\N":
        return None
    out = bytearray()
    i = 0
    raw = field.encode("utf-8")
    while i < len(raw):
        if raw[i] != 0x5C:
            out.append(raw[i])
            i += 1
            continue
        if i + 1 >= len(raw):
            raise ValueError(f"dangling escape in {field!r}")
        n = raw[i + 1]
        if n == 0x5C:
            out.append(0x5C)
        elif n == ord("n"):
            out.append(0x0A)
        elif n == ord("r"):
            out.append(0x0D)
        elif n == ord("t"):
            out.append(0x09)
        elif n == ord("x"):
            out.append(int(raw[i + 2 : i + 4].decode("ascii"), 16))
            i += 4
            continue
        else:
            raise ValueError(f"bad escape in {field!r}")
        i += 2
    return out.decode("utf-8")


def parse_trace(text: str) -> dict[tuple[str, str, str], dict]:
    """Map (section, config, case-or-api) records.

    File parses are keyed by (file, config, api) and hold handler rows plus code.
    String/alloc cases are keyed by (string|alloc, config, case).
    """
    section = ""
    config = ""
    case = ""
    out: dict[tuple[str, str, str], dict] = {}

    def slot(kind: str, cfg: str, name: str) -> dict:
        key = (kind, cfg, name)
        if key not in out:
            out[key] = {"h": [], "a": [], "code": None}
        return out[key]

    for line in text.splitlines():
        parts = line.split("\t")
        tag = parts[0]
        if tag == "S":
            section = parts[1]
            config = ""
            case = ""
        elif tag == "K":
            config = parts[1]
            case = ""
        elif tag == "C":
            config = parts[1]
            case = parts[2]
        elif tag == "H":
            _cfg, api, lineno, rc, section_f, name_f, value_f = parts[1:]
            kind = section
            name = case if section in ("string", "alloc") else api
            row = slot(kind, _cfg, name)
            row["h"].append(
                {
                    "lineno": None if lineno == "-" else int(lineno),
                    "rc": int(rc),
                    "section": unescape(section_f),
                    "name": unescape(name_f),
                    "value": unescape(value_f),
                }
            )
        elif tag == "A":
            kind = section
            name = case if section in ("string", "alloc") else "parse"
            row = slot(kind, config, name)
            row["a"].append(parts[1:])
        elif tag == "R":
            _cfg, api, code, _user = parts[1:]
            kind = section
            name = case if section in ("string", "alloc") else api
            row = slot(kind, _cfg, name)
            row["code"] = int(code)
    return out


def file_dumper(events: list[dict], lineno: bool) -> list[str]:
    prev = ""
    lines: list[str] = []
    for ev in events:
        section = ev["section"] or ""
        name = ev["name"]
        value = ev["value"]
        if name is None or section != prev:
            lines.append(f"... [{section}]\n")
            prev = section[:49]
        if name is None:
            continue
        if lineno:
            if value is None:
                lines.append(f"... {name};  line {ev['lineno']}\n")
            else:
                lines.append(f"... {name}={value};  line {ev['lineno']}\n")
        elif value is None:
            lines.append(f"... {name};\n")
        else:
            lines.append(f"... {name}={value};\n")
    return lines


def string_dumper(events: list[dict]) -> list[str]:
    prev = ""
    lines: list[str] = []
    for ev in events:
        section = ev["section"] or ""
        if section != prev:
            lines.append(f"... [{section}]\n")
            prev = section[:49]
        lines.append(f"... {ev['name']}={ev['value']};\n")
    return lines


def alloc_dumper(record: dict) -> list[str]:
    lines: list[str] = []
    prev = ""
    hi = 0
    events = record["h"]
    for op in record["a"]:
        if op[0] == "malloc":
            lines.append(f"ini_malloc({op[1]})\n")
        elif op[0] == "realloc":
            lines.append(f"ini_realloc({op[1]})\n")
        elif op[0] == "free":
            lines.append("ini_free()\n")
        else:
            raise ValueError(op)
    # Alloc events are interleaved with handlers in the oracle, but this
    # function is only used when A lines were collected separately. The caller
    # passes a merged stream instead.
    del hi, prev, events
    return lines


def merged_alloc(text_lines: list[str], config: str, case: str) -> list[str]:
    """Replay A and H lines in order for one alloc case."""
    prev = ""
    out: list[str] = []
    section = ""
    seeing = False
    for line in text_lines:
        parts = line.split("\t")
        tag = parts[0]
        if tag == "S":
            section = parts[1]
            seeing = section == "alloc"
        elif tag == "C" and seeing and parts[1] == config and parts[2] == case:
            prev = ""
        elif tag == "A" and seeing and config == "alloc":
            # Only the lines after the matching C and before the next C/K/S.
            if parts[1] == "malloc":
                out.append(f"ini_malloc({parts[2]})\n")
            elif parts[1] == "realloc":
                out.append(f"ini_realloc({parts[2]})\n")
            elif parts[1] == "free":
                out.append("ini_free()\n")
        elif tag == "H" and seeing and parts[1] == config:
            ev_section = unescape(parts[5]) or ""
            name = unescape(parts[6])
            value = unescape(parts[7])
            if ev_section != prev:
                out.append(f"... [{ev_section}]\n")
                prev = ev_section[:49]
            out.append(f"... {name}={value};\n")
        elif tag in ("K", "C", "S") and out:
            break
    return out


def load_oracle() -> dict[str, str]:
    traces: dict[str, str] = {}
    for name in FILES:
        if name == "no_file.ini":
            continue
        path = ROOT / "tests" / name
        proc = subprocess.run(
            [str(ORACLE), str(path)],
            cwd=ROOT,
            check=True,
            capture_output=True,
        )
        if proc.stderr:
            sys.stderr.write(proc.stderr.decode())
        traces[name] = proc.stdout.decode("utf-8")
    return traces


def main() -> int:
    if not ORACLE.is_file():
        print("build/oracle is missing; run make oracle", file=sys.stderr)
        return 1
    traces = load_oracle()
    parsed = {name: parse_trace(text) for name, text in traces.items()}
    failed = 0

    for cfg in FILE_CONFIGS:
        chunks: list[str] = []
        user = 0
        u = 100
        for fname in FILES:
            if fname == "no_file.ini":
                chunks.append(f"{fname}: e=-1 user={user}\n")
                u += 1
                continue
            rec = parsed[fname][("file", cfg, "parse")]
            if rec["code"] is None:
                print(f"missing result {fname} {cfg}")
                return 1
            body = file_dumper(rec["h"], cfg == "handler_lineno")
            if rec["h"]:
                user = u
            chunks.extend(body)
            chunks.append(f"{fname}: e={rec['code']} user={user}\n")
            u += 1
        got = "".join(chunks).encode()
        expected = (ROOT / "tests" / f"baseline_{cfg}.txt").read_bytes()
        if got != expected:
            failed += 1
            print(f"DIFF baseline_{cfg}.txt")
            Path(f"/tmp/got_{cfg}.txt").write_bytes(got)
            subprocess.run(["diff", "-u", str(ROOT / "tests" / f"baseline_{cfg}.txt"), f"/tmp/got_{cfg}.txt"])
        else:
            print(f"ok baseline_{cfg}.txt")

    for cfg in ("string", "heap_string"):
        chunks = []
        user = 0
        u = 100
        sample = next(iter(parsed.values()))
        for case in STRING_CASES:
            rec = sample[("string", cfg, case)]
            body = string_dumper(rec["h"])
            if rec["h"]:
                user = u
            chunks.extend(body)
            chunks.append(f"{case}: e={rec['code']} user={user}\n")
            u += 1
        got = "".join(chunks).encode()
        expected = (ROOT / "tests" / f"baseline_{cfg}.txt").read_bytes()
        if got != expected:
            failed += 1
            print(f"DIFF baseline_{cfg}.txt")
            Path(f"/tmp/got_{cfg}.txt").write_bytes(got)
            subprocess.run(["diff", "-u", str(ROOT / "tests" / f"baseline_{cfg}.txt"), f"/tmp/got_{cfg}.txt"])
        else:
            print(f"ok baseline_{cfg}.txt")

    sample_text = next(iter(traces.values())).splitlines()
    # Re-read alloc lines from one full trace. merged_alloc stops too early if
    # it treats the first K as the end; pass only the alloc block.
    alloc_lines = []
    keep = False
    for line in sample_text:
        if line == "S\talloc":
            keep = True
        elif line.startswith("S\t") and keep:
            break
        if keep:
            alloc_lines.append(line)
    body = []
    prev = ""
    code = None
    for line in alloc_lines:
        parts = line.split("\t")
        tag = parts[0]
        if tag == "A":
            if parts[1] == "malloc":
                body.append(f"ini_malloc({parts[2]})\n")
            elif parts[1] == "realloc":
                body.append(f"ini_realloc({parts[2]})\n")
            elif parts[1] == "free":
                body.append("ini_free()\n")
        elif tag == "H":
            section = unescape(parts[5]) or ""
            name = unescape(parts[6])
            value = unescape(parts[7])
            if section != prev:
                body.append(f"... [{section}]\n")
                prev = section[:49]
            body.append(f"... {name}={value};\n")
        elif tag == "R":
            code = int(parts[3])
    body.append(f"basic: e={code}\n")
    got = "".join(body).encode()
    expected = (ROOT / "tests" / "baseline_alloc.txt").read_bytes()
    if got != expected:
        failed += 1
        print("DIFF baseline_alloc.txt")
        Path("/tmp/got_alloc.txt").write_bytes(got)
        subprocess.run(["diff", "-u", str(ROOT / "tests" / "baseline_alloc.txt"), "/tmp/got_alloc.txt"])
    else:
        print("ok baseline_alloc.txt")

    print(f"{15 - failed} baselines matched, {failed} differed")
    return 1 if failed else 0


if __name__ == "__main__":
    sys.exit(main())
