# Parity

C `ini.c` / `ini.h` are unchanged. Drivers follow `tools/DRIVER_FORMAT.md`. Section `parse` only.

## Result

No driver divergences. `make parity-check` is identical on all 12 `tests/*.ini` fixtures, and `build/oracle-ffi` matches `build/oracle-default` on each of them. `make hook-trace` matches for the default ABI (C, FFI, Rust) and for `custom_alloc` fail-on-Nth (C and Rust).

## Per-fixture

Every fixture was compared for all 13 configs and all five entry points (`file`, `fileptr`, `stream`, `string`, `length`).

| Fixture | Driver result |
|---------|----------------|
| `tests/bad_comment.ini` | identical |
| `tests/bad_multi.ini` | identical |
| `tests/bad_section.ini` | identical |
| `tests/bom.ini` | identical |
| `tests/duplicate_sections.ini` | identical |
| `tests/long_line.ini` | identical |
| `tests/long_section.ini` | identical |
| `tests/multi_line.ini` | identical |
| `tests/name_only_after_error.ini` | identical |
| `tests/no_value.ini` | identical |
| `tests/normal.ini` | identical |
| `tests/user_error.ini` | identical |

Ported `unittest*.c` suites match `tests/baseline_*.txt` (`make c-tests` on the C side, `cargo test -p inih-core` on the Rust side).

## Quirks matched

| Behavior | C |
|----------|---|
| `isspace` is the C locale set (space, tab, LF, VT, FF, CR). `*--end` in the `&&` is Sonar `c:S912` (maintainability only) | `ini.c:49` |
| Inline comment only after whitespace | `ini.c:67-72` |
| `strncpy`-style truncation of section and name to `size - 1` | `ini.c:84-91`, `ini.c:207`, `ini.c:236` |
| Blank line is a comment because `strchr` finds the prefix string's NUL | `ini.c:186` |
| Continuation only when `prev_name` is set and `start > line` | `ini.c:190` |
| Over-long line still parses the buffered prefix; the rest is discarded 16 bytes at a time and the line number is the error | `ini.c:165-174` |
| Heap buffer starts at `INI_INITIAL_ALLOC` and doubles up to `INI_MAX_LINE` when realloc is enabled | `ini.c:146-160` |
| UTF-8 BOM skipped only on line 1 | `ini.c:177-182` |
| Handler return 0 records the line but parsing continues unless `INI_STOP_ON_FIRST_ERROR` | `ini.c:239`, `ini.c:256-258` |
| No-value calls the handler with a NULL value; new-section calls use NULL name and value | `ini.c:211-213`, `ini.c:244-248` |
| `ini_parse` returns `-1` when `fopen` fails and `-2` when the line buffer cannot be allocated | `ini.c:130-132`, `ini.c:282-283` |

## Divergences

None.

## Oracle defect found

None. AddressSanitizer and UBSan were clean.

Command (`ASAN_CC=gcc` because `cc` is Clang 18 without the compiler-rt archive):

```sh
make asan-oracle
```

That builds every oracle variant with:

```sh
gcc -g -fsanitize=address,undefined -fno-omit-frame-pointer <config -D flags> -I. tools/inih-oracle.c ini.c -o build/asan/oracle-<config>
```

and runs `ASAN_OPTIONS=detect_leaks=0` on every `tests/*.ini`. It also runs `tests/unittest.c` (default flags) and `tools/hook-trace.c` (`custom_alloc`, `--fail 0` and `--fail 2`) under the same sanitizers. Output: `asan oracle <config> ok` for all 13 configs, `asan unittest_multi ok`, `asan hook-custom ok`, `asan hook-custom fail 2 ok`. No `ASAN:` lines.

## Known gaps

- stderr is not compared (`compare_stderr` is false).
- Fixtures are the 12 `tests/*.ini` files. The string and alloc unit-test literals are covered by the ported Rust tests and by `make c-tests`, not by the gate globs.
- Linux `fopen("r")` is byte I/O. Windows text-mode translation is not in this environment.
- C++ `INIReader` and `fuzzing/` are out of scope (`MIGRATION.md`).
- `inih-ffi` is the default header ABI. Other `-D` configs are core presets plus separate C oracle binaries.

## Gate report

```
# Parity report

Result: PASS. 12 fixtures, 12 compared: 12 identical, 0 logged exceptions, 0 diverged; 0 gate problems.

## Method

- Workspace: `/agent/repos/inih1`
- Tree hash: `f48de75c63982136e4b36df13f50b220e449eb3035b2804fe9ddf8aba4f17663`
- Build: `make parity-build` (exit 0)
- C oracle: `./build/inih-oracle {input}`
- Rust port: `./target/release/inih-driver {input}`
- Input: fixture path substituted for `{input}`
- Compared: stdout bytes and exit status (stderr not compared)
- Fixture globs: `tests/*.ini` (12 files)
- Exceptions file: `PARITY_EXCEPTIONS.md`
- Exception tests: not run (no exception rows)

## Per-fixture results

| Fixture | Result | C status | Rust status | C stdout sha256 | Rust stdout sha256 |
|---|---|---|---|---|---|
| `tests/bad_comment.ini` | identical | exit 0 | exit 0 | `f9842a58f8fb` | `f9842a58f8fb` |
| `tests/bad_multi.ini` | identical | exit 0 | exit 0 | `2bc6e4d5248b` | `2bc6e4d5248b` |
| `tests/bad_section.ini` | identical | exit 0 | exit 0 | `016de4cb6238` | `016de4cb6238` |
| `tests/bom.ini` | identical | exit 0 | exit 0 | `e2a0b20101f3` | `e2a0b20101f3` |
| `tests/duplicate_sections.ini` | identical | exit 0 | exit 0 | `bc2a1dd5075d` | `bc2a1dd5075d` |
| `tests/long_line.ini` | identical | exit 0 | exit 0 | `bb0218c06370` | `bb0218c06370` |
| `tests/long_section.ini` | identical | exit 0 | exit 0 | `cbd624c284fd` | `cbd624c284fd` |
| `tests/multi_line.ini` | identical | exit 0 | exit 0 | `eb7525f1d3eb` | `eb7525f1d3eb` |
| `tests/name_only_after_error.ini` | identical | exit 0 | exit 0 | `359e48af34e9` | `359e48af34e9` |
| `tests/no_value.ini` | identical | exit 0 | exit 0 | `13181a8593b0` | `13181a8593b0` |
| `tests/normal.ini` | identical | exit 0 | exit 0 | `59ad7784bd84` | `59ad7784bd84` |
| `tests/user_error.ini` | identical | exit 0 | exit 0 | `4ce65663cf7e` | `4ce65663cf7e` |

## Divergences

None.

## Exceptions used

None.

## Gate problems

None.

## Pins

- 10 oracle files pinned
- 12 fixtures pinned

## Not covered

- Inputs outside the fixture globs; the gate proves parity only on the listed fixtures.
- stderr output (set `compare_stderr` to include it).
- Independence of the Rust driver: the gate only checks that it is not the same executable as the C driver.
```
