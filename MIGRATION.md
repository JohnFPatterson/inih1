# Migration

The inih parser is a safe Rust core plus a default-ABI C shim. The oracle is a SAX handler trace plus the `ini_parse*` return code, not a reprint of the file. Both drivers print the format in `tools/DRIVER_FORMAT.md`.

## Layout

| Path | Role |
|------|------|
| `ini.c`, `ini.h` | Unmodified C spec |
| `inih-core` | Parser, `Config`, `Error`. `forbid(unsafe_code)` |
| `inih-ffi` | Default `ini.h` ABI only (`staticlib` + `cdylib`) |
| `inih-driver` | Rust driver (`target/release/rust-driver`) |
| `tools/inih-oracle.c` | One binary; one translation unit per `tests/unittest.sh` configuration |
| `tools/hook-trace.c` | Default-ABI handler and reader trace, linked to `ini.c` and to `libinih_ffi.a` |
| `.cursor/parity.json` | Single `c_cmd` / `rust_cmd`. No modules |

`inih-ffi` exports `ini_parse`, `ini_parse_file`, `ini_parse_stream`, `ini_parse_string`, and `ini_parse_string_length` with the default handler (no `lineno`). `INI_API` in `ini.h` is a visibility attribute, not a type wrapper. `tools/header_fns.py` extracts the five `int name(` prototypes. `make exports` checks those symbols on `libinih_ffi.a`.

The line-number callback (`INI_HANDLER_LINENO`) and the custom allocator (`INI_CUSTOM_ALLOCATOR`) stay on `inih-core` and on the C oracle. They change the C signature, so they are not default-ABI exports.

## Out of scope

Named and not ported:

- `cpp/INIReader` and the rest of `cpp/`
- `examples/`
- `fuzzing/`

## Unsafe

`rg -n "unsafe" --glob '*.rs'` (2026-10-01):

- `inih-core` has `#![forbid(unsafe_code)]` only. No `unsafe` blocks.
- Every executable `unsafe` is in `inih-ffi/src/lib.rs`, and each block has an adjacent `// SAFETY:` comment. The `unsafe extern "C"` functions document `# Safety`.
- `inih-ffi/tests/abi.rs` uses `unsafe` only to call those exports.

## SonarQube

Method: SonarScanner CLI 8.1.0.6389, local Java 21, `sonar.cfamily.compile-commands` for `ini.c` (`clang -std=c11`), Rust enterprise sensor on `inih-core/src`, `inih-ffi/src`, `inih-driver/src`, and the two test trees. Uploaded to the linked SonarCloud project. The MCP namespace `Sonarqube` was ready and was used to confirm the project has a single long-lived `master` branch. It does not expose `analyze_file_list` or `run_advanced_code_analysis`. `sonar analyze` (Vortex) returned 403, not entitled.

The report processed successfully. Query: `GET /api/issues/search` with `resolved=false` (1 issue) and `GET /api/hotspots/search` with `status=TO_REVIEW` (0). The analyzed `ini.c` is the current file; Sonar line 49 is current line 49.

| Path | SECURITY (OPEN/CONFIRMED) | Hotspots `TO_REVIEW` | Other |
|------|---------------------------|----------------------|-------|
| `ini.c` | 0 | 0 | 1 maintainability: `c:S912` at `ini.c:49` |
| `ini.h` | 0 | 0 | 0 |
| `inih-core`, `inih-ffi`, `inih-driver` | 0 | 0 | 0 |

`c:S912` is the side effect in `isspace((unsigned char)(*--end))` on the right-hand side of `&&` (`ini.c:49`). Impact is MAINTAINABILITY, not SECURITY. Rust keeps the same rstrip order. No `PE-NNN`.

### Rust Sonar mitigations

None. The scan reported no open SECURITY issues and no `TO_REVIEW` hotspots on the new Rust.

### CI quality gate

The project quality gate status after this analysis is `NONE` (no conditions). This repository has no Sonar CI workflow. The intended gate, when one is attached, fails on new-code security for `inih-core` and `inih-ffi`. It does not fail the port because of legacy C findings. The `c:S912` maintainability issue stays on the C oracle.

## Parity exceptions

None. See `PARITY_EXCEPTIONS.md`.

## Behavior kept on purpose

- All 15 `tests/unittest.sh` configurations, selected at runtime by `Config::preset` and by a matching C translation unit in the oracle.
- C-locale `isspace` only (space, tab, LF, VT, FF, CR).
- Inline comments only after whitespace (`ini.c:65-72`).
- A blank line is a comment because `strchr` on the comment prefixes matches the NUL (`ini.c:186`).
- Section and name truncated to `max-1` (`ini.c:84`, `ini.c:207`, `ini.c:236`).
- Handler return 0 records the first error line and parsing continues unless `stop_on_first_error` (`ini.c:256-258`).
- Over-long lines: grow when realloc is enabled, then discard the rest through a 16-byte buffer and set the error to that line (`ini.c:165-173`). A line of exactly `max-1` bytes with no newline is an error only if another read returns data.
- UTF-8 BOM stripped only on line 1 (`ini.c:177-182`).
- Continuation only when a previous name exists, the line is non-empty, and it has leading whitespace (`ini.c:190`).
- Heap vs stack output matches when the limits match. Stack mode does not emit `A` lines. The line buffer is a Rust `Vec`; that allocation is not a C `ini_malloc` and is not hook-visible on the default ABI.
- Custom-allocator trace (`A` lines) follows `ini_malloc` / `ini_realloc` / `ini_free`, including free-after-failed-realloc (`ini.c:152-154`). `--alloc-fail N` counts malloc and realloc only.

## Behavior changed on purpose

| ID | C behavior | Rust behavior | Approval | Pinning test |
|----|------------|---------------|----------|--------------|
| CH-001 | `ini_parse_stream` asserts on a null reader, stream, or handler (`ini.c:124-126`). `fopen` of a null path and a null `FILE*` are undefined. | The default-ABI shim returns `-1` for a null filename, file, reader, stream, or handler. `ini_parse_string(NULL)` returns `-1`. `ini_parse_string_length(NULL, 0)` returns `0` (empty input, same as a zero-length read). `ini_parse_string_length(NULL, n)` for `n > 0` returns `-1`. | Port request, 2026-10-01. Malformed input must return an error and must not crash. These C paths assert or are undefined, so the shim cannot match them. | `inih-ffi/tests/abi.rs::null_callbacks_return_error` |

Copies of section, name, and value are passed to the C handler. The documented contract is unchanged: the pointers are valid only for the call, the value may be mutated, and section and name must not be.

`lineno` uses `checked_add` and returns `-2` if it would overflow (`inih-core/src/parse.rs`). C `lineno++` (`ini.c:163`) is signed overflow if it wraps. No fixture reaches that line count, and the drivers do not compare it.

## Known gaps

- `parse_path` uses `std::fs::read`. A directory fails that read and returns `-1`. C `ini_parse` uses `fopen`, which can succeed on a directory and then fail inside `fgets`. The FFI `ini_parse` / `ini_parse_file` path uses `fopen`. No fixture is a directory.
- The parity gate fixtures are `tests/*.ini` (12 files). `tests/no_file.ini` is not in the tree. The ported unittest still parses that path and compares `e=-1` against `baseline_*.txt`.
