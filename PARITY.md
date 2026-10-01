# Parity

12 fixtures, 12 identical.

Method: the parity gate built both drivers (`make drivers`) and compared stdout bytes and exit status of `./build/oracle {input}` and `./target/release/rust-driver {input}` on every `tests/*.ini` file. stderr is not compared. Reproduce:

```sh
printf '%s' '{"status":"completed","loop_count":0,"workspace_roots":["/workspace"]}' | ./.cursor/hooks/c-rust-parity/parity_gate.py --force
```

The gate report below is `build/parity-gate/parity-report.md` from that run (exit 0). Pins: `ini.c`, `ini.h`, `tests/unittest.c`, `tests/unittest_alloc.c`, `tests/unittest_string.c`, and the 12 fixtures.

## Per-input results

| Fixture | Result | C status | Rust status | stdout sha256 |
|---------|--------|----------|-------------|---------------|
| `tests/bad_comment.ini` | identical | 0 | 0 | `9314941e5f19` |
| `tests/bad_multi.ini` | identical | 0 | 0 | `d20978431bc6` |
| `tests/bad_section.ini` | identical | 0 | 0 | `29233c875b6b` |
| `tests/bom.ini` | identical | 0 | 0 | `430517357376` |
| `tests/duplicate_sections.ini` | identical | 0 | 0 | `fd3ba005af88` |
| `tests/long_line.ini` | identical | 0 | 0 | `54f069a0ee39` |
| `tests/long_section.ini` | identical | 0 | 0 | `85b589acf3c1` |
| `tests/multi_line.ini` | identical | 0 | 0 | `7864429ac3c3` |
| `tests/name_only_after_error.ini` | identical | 0 | 0 | `75f079655fb6` |
| `tests/no_value.ini` | identical | 0 | 0 | `a8d0c690472a` |
| `tests/normal.ini` | identical | 0 | 0 | `a81b13bb61b4` |
| `tests/user_error.ini` | identical | 0 | 0 | `98da33bcea19` |

## Quirks kept

| Quirk | C |
|-------|---|
| Whitespace is C-locale `isspace` only | `ini.c:49`, `ini.c:57` |
| Inline comment only after whitespace | `ini.c:65-72` |
| Blank line is a comment (`strchr` matches NUL) | `ini.c:186` |
| Section and name truncated to `max-1` | `ini.c:84`, `ini.c:207`, `ini.c:236` |
| Handler `0` records the first error and parsing continues | `ini.c:239-240` |
| `stop_on_first_error` breaks after that | `ini.c:256-258` |
| Over-long line is parsed truncated, then the rest is discarded; error is that line | `ini.c:165-173` |
| UTF-8 BOM stripped only on line 1 | `ini.c:177-182` |
| Continuation requires a previous name, a non-empty line, and leading whitespace | `ini.c:190` |
| New-section callback gets null name and value | `ini.c:211-213` |
| Name with no value is an error unless `allow_no_value` | `ini.c:244-251` |
| Failed realloc frees the line and returns `-2` | `ini.c:152-154` |
| Null reader, stream, or handler is asserted in C | `ini.c:124-126` |

## Divergences

None.

## Exceptions

None.

## Oracle defect found

None.

## Behavior kept on purpose

The 15 `tests/unittest.sh` configurations, the SAX trace, and the quirks in the table above. `python3 tools/check_baselines.py` rebuilds each `baseline_*.txt` from the oracle trace. `inih-core/tests/unittest_port.rs` compares the Rust parser to those baselines, including `tests/no_file.ini` (open error `-1`).

## Behavior changed on purpose

CH-001 only. The default-ABI shim returns `-1` for null filename, file, reader, stream, or handler, and for a null string with a positive length. `ini_parse_string_length(NULL, 0)` returns `0`. C asserts or is undefined on those paths (`ini.c:124-126`). Pinning test: `inih-ffi/tests/abi.rs::null_callbacks_return_error`. Approval: port request, 2026-10-01. Details in `MIGRATION.md`.

`lineno` overflow returns `-2` from the core. No fixture reaches it.

## Not covered

- `cpp/INIReader`, `examples/`, and `fuzzing/` (named out of scope).
- Opening a directory with `ini_parse`. The core `parse_path` uses `std::fs::read`. The FFI path uses `fopen`.
- stderr of the drivers.
- Gate fixtures do not include a missing path. The ported unittest does (`no_file.ini` in `baseline_*.txt`).

## ASan

Command: `make asan` (instrumented oracle over every `tests/*.ini`, instrumented `hook-trace` over every fixture, and all 15 `unittest.sh` configurations diffed against `baseline_*.txt`, with `ASAN_OPTIONS=detect_leaks=1:halt_on_error=1`).

```
tests/unittest_alloc.c:29:18: warning: unused parameter 'user' [-Wunused-parameter]
   29 | int dumper(void* user, const char* section, const char* name,
      |                  ^
1 warning generated.
asan: clean
```

Exit 0. The warning is the pre-existing `tests/unittest_alloc.c` dumper parameter. `ini.c` was not edited.

## Hook-trace

`make hook-trace` links `tools/hook-trace.c` to `ini.c` and to `target/release/libinih_ffi.a`, diffs them on every `tests/*.ini`, and diffs `--alloc-fail 0..4` for `--sections alloc` on `tests/normal.ini` and `--sections allocfile` on `tests/long_line.ini`. Result: `hook-trace: clean`.

## Gate report

```
# Parity report

Result: PASS. 12 fixtures, 12 compared: 12 identical, 0 logged exceptions, 0 diverged; 0 gate problems.

## Method

- Workspace: `/workspace`
- Tree hash: `d8db79b3f011b564ebe60d238ec81e15d71dd7510c73a044a15ac1c0c1ed6113`
- Build: `make drivers` (exit 0)
- C oracle: `./build/oracle {input}`
- Rust port: `./target/release/rust-driver {input}`
- Input: fixture path substituted for `{input}`
- Compared: stdout bytes and exit status (stderr not compared)
- Fixture globs: `tests/*.ini` (12 files)
- Exceptions file: `PARITY_EXCEPTIONS.md`
- Exception tests: not run (no exception rows)

Reproduce:

    printf '%s' '{"status": "completed", "loop_count": 0, "workspace_roots": ["/workspace"]}' | '/workspace/.cursor/hooks/c-rust-parity/parity_gate.py' --force

## Per-fixture results

| Fixture | Result | C status | Rust status | C stdout sha256 | Rust stdout sha256 |
|---|---|---|---|---|---|
| `tests/bad_comment.ini` | identical | exit 0 | exit 0 | `9314941e5f19` | `9314941e5f19` |
| `tests/bad_multi.ini` | identical | exit 0 | exit 0 | `d20978431bc6` | `d20978431bc6` |
| `tests/bad_section.ini` | identical | exit 0 | exit 0 | `29233c875b6b` | `29233c875b6b` |
| `tests/bom.ini` | identical | exit 0 | exit 0 | `430517357376` | `430517357376` |
| `tests/duplicate_sections.ini` | identical | exit 0 | exit 0 | `fd3ba005af88` | `fd3ba005af88` |
| `tests/long_line.ini` | identical | exit 0 | exit 0 | `54f069a0ee39` | `54f069a0ee39` |
| `tests/long_section.ini` | identical | exit 0 | exit 0 | `85b589acf3c1` | `85b589acf3c1` |
| `tests/multi_line.ini` | identical | exit 0 | exit 0 | `7864429ac3c3` | `7864429ac3c3` |
| `tests/name_only_after_error.ini` | identical | exit 0 | exit 0 | `75f079655fb6` | `75f079655fb6` |
| `tests/no_value.ini` | identical | exit 0 | exit 0 | `a8d0c690472a` | `a8d0c690472a` |
| `tests/normal.ini` | identical | exit 0 | exit 0 | `a81b13bb61b4` | `a81b13bb61b4` |
| `tests/user_error.ini` | identical | exit 0 | exit 0 | `98da33bcea19` | `98da33bcea19` |

## Divergences

None.

## Exceptions used

None.

## Gate problems

None.

## Pins

- 5 oracle files pinned on this first run
- 12 fixtures pinned

## Not covered

- Inputs outside the fixture globs; the gate proves parity only on the listed fixtures.
- stderr output (set `compare_stderr` to include it).
- Independence of the Rust driver: the gate only checks that it is not the same executable as the C driver.
```
