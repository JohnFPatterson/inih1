# inih C to Rust

## Layout

| Path | Role |
|------|------|
| `ini.c`, `ini.h` | Unmodified C oracle |
| `inih-core` | All parser logic. `#![forbid(unsafe_code)]`. `Config` carries the `ini.h` macros |
| `inih-ffi` | Default header ABI only (`INI_HANDLER_LINENO=0` and the other header defaults). `staticlib` + `cdylib` + `rlib` |
| `tools/inih-oracle.c` | Public-header driver, compiled once per `tests/unittest.sh` config |
| `tools/inih-dispatch.c` | `build/inih-oracle`: runs every variant |
| `tools/hook-trace.c` | Handler and allocator trace |
| `inih-core/src/bin/inih-driver.rs` | Rust driver, same stdout as the C oracle |
| `inih-core/src/bin/inih-hook-trace.rs` | Rust hook trace |

`inih-ffi` exports `ini_parse`, `ini_parse_file`, `ini_parse_stream`, `ini_parse_string`, and `ini_parse_string_length`. Non-default flags are `inih_core::configs()` presets, not extra C symbols.

## Oracle shape

Text parse plus callback trace. There is no printer.

Driver section: `parse`. For each `tests/*.ini` fixture the drivers print `rc` and one line per `ini_handler` call (`section`, `name`, `value`, and `lineno` only for `handler_lineno`). Both sides run the same config list and the five public entry points. Format: `tools/DRIVER_FORMAT.md`.

## Out of scope

- C++ `INIReader` (`cpp/INIReader.h`, `cpp/INIReader.cpp`, `examples/INIReaderExample.cpp`, `examples/INIReaderExampleErrors.cpp`, `examples/cpptest.sh`)
- AFL fuzzer (`fuzzing/`)

## Unsafe

`rg -n unsafe --glob '*.rs' -g '!target/**'`:

- `inih-core` (lib and both bins): only `#![forbid(unsafe_code)]`
- `inih-ffi/src/lib.rs`: every `unsafe` block has a `SAFETY:` comment. Crate denies `unsafe_op_in_unsafe_fn` and `clippy::undocumented_unsafe_blocks`
- `inih-ffi/tests/abi.rs`: tests call the C ABI

## Export check

Header pattern is `INI_API int name(`, not a `INI_API_PUBLIC(type)` wrapper. `make exports` keeps the five names from `ini.h` and checks `nm` on `target/release/libinih_ffi.a`. `comm` is empty.

## Tests

`tests/unittest.c`, `unittest_string.c`, and `unittest_alloc.c` are ported case for case in `inih-core/tests/`. Each `unittest.sh` / `tests/meson.build` variant compares to its `baseline_*.txt`.

No case was dropped. Public entry points:

- Default `unittest.c`, `examples/ini_dump.c`, `examples/ini_example.c`, and `examples/ini_xmacros.c` link against `ini.c` and against `libinih_ffi.a` (`make c-tests`, `make ffi-tests`). Output matches.
- Variant `-D` suites stay on the C side as oracles and on the Rust side as `Config` presets. The default FFI binary cannot implement those macros.

`inih-ffi/tests/abi.rs` checks callback bytes, in-call mutation of `value` (it must not rewrite the caller buffer), explicit length, handler return 0, missing file `-1`, `ini_parse_file`, `ini_parse_stream`, and empty `ini_parse_string_length`. The API returns `int` only; there is no library free.

## Hook trace

`make hook-trace` diffs `tools/hook-trace.c` against `inih-ffi` and against `inih-hook-trace`.

| Run | Result |
|-----|--------|
| `default`, `--fail 0` and `--fail 1` | C, `inih-ffi`, and Rust identical (stack buffer, no `malloc`) |
| `custom_alloc` (`INI_CUSTOM_ALLOCATOR`, heap, realloc, initial 12), `--fail 0` through `--fail 8` | C and Rust identical, including fail-on-Nth `M`/`R`/`F` order |

The default FFI does not call `ini_malloc`. Heap and realloc traces are the core `AllocSink` path, checked against the C hook binary and against `tests/baseline_alloc.txt`.

## Parity exceptions (`PE-NNN`)

None. `PARITY_EXCEPTIONS.md` has no rows.

## Behavior changed on purpose (`CH-NNN`)

None.

| ID | Change | Reason | Approval | Pinning test |
|----|--------|--------|----------|--------------|
| — | — | — | — | — |

A Rust `Config` with `max_line < 2`, `max_section < 1`, `max_name < 1`, or heap `initial_alloc < 2` returns `-2` instead of underflowing the way a broken C macro would. The header defaults and every `unittest.sh` preset are valid, and `inih-ffi` hardcodes those defaults, so this is not a compared behavior change.

## SonarQube

SonarQube Cloud, long-lived branch `master`, analysis `2026-10-01T04:41:14Z` (project key from the environment, not stored here). `ini.c` was not edited after that analysis. Sonar line 49 is still `ini.c:49`.

| Item | Result |
|------|--------|
| SECURITY issues on `ini.c` / `ini.h` (OPEN or CONFIRMED) | none |
| Security hotspots `TO_REVIEW` on `ini.c` / `ini.h` | none |
| Other open issue | `c:S912` at `ini.c:49`, type CODE_SMELL, impact MAINTAINABILITY only (`*--end` in `ini_rstrip`). Not a `PE-NNN`. Rust `rstrip` matches the bytes |

Rechecked on 2026-10-01 after the port commit, using the project key from the environment and not storing it here. `list_branches` returns only long-lived `master` (analysis `2026-10-01T04:41:14+0000`); `list_pull_requests` returns 0. `search_sonar_issues_in_projects` with `SECURITY` and statuses OPEN/CONFIRMED returns 0. `search_security_hotspots` with `TO_REVIEW` returns 0. The only open issue is `c:S912` (`show_rule`: type CODE_SMELL, impact MAINTAINABILITY). The local `Sonarqube` MCP namespace is in error (`mcp_auth` has no authentication URL). `sonar analyze` on `ini.c`, `ini.h`, and the new Rust files returns 403: Vortex analysis is not available on this connection. There is still no analysis of this branch's new Rust, so no Rust SECURITY issue or `TO_REVIEW` hotspot was returned.

### Rust Sonar mitigations

None recorded: the scan above did not report any on the new Rust.

### CI / quality gate

`get_project_quality_gate_status` for `master` is `NONE` (no conditions). The existing `.github/workflows/tests.yml` job diffs C `unittest.sh` output and does not fail because of a Sonar rating on `ini.c`. `.github/workflows/rust.yml` runs `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`, and `cargo test` for the port. A future Sonar gate should fail on new-code / Rust security, not on the legacy C rating alone.

## Build

`make parity-build` compiles every C oracle variant, `build/inih-oracle`, `build/oracle-ffi`, and `target/release/inih-driver`. `make parity` runs the parity gate.
