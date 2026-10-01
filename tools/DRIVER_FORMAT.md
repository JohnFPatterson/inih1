# Driver format

Both `build/inih-oracle` (C) and `target/release/inih-driver` (Rust) print this text to stdout. Exit status is part of the parity compare. stderr is not compared.

The library has no printer. The only section is `parse`: text parse plus the `ini_handler` callback trace.

## Invocation

```
inih-oracle [--sections <name>[,<name>…]] <fixture>
inih-driver [--sections <name>[,<name>…]] <fixture>
```

`<fixture>` is a path to an INI file. `--sections` may appear before or after the path.

Without `--sections`, print every section. The only section name is `parse`. An unknown section name exits `2` and prints `unknown section` to stderr. A missing path argument exits `2`.

## Section `parse`

The C oracle is `tools/inih-oracle.c` compiled once per config below and linked with `ini.c`. `build/inih-oracle` runs those binaries in order. The Rust driver uses the same configs in the same order. Each config runs five public entry points on the fixture bytes:

| api | C function |
|-----|------------|
| `file` | `ini_parse` |
| `fileptr` | `ini_parse_file` |
| `stream` | `ini_parse_stream` with an `fgets`-style reader |
| `string` | `ini_parse_string` |
| `length` | `ini_parse_string_length` (full file length, including bytes after an embedded NUL) |

If the path cannot be opened, every api prints `rc -1` and no calls, and the process still exits `0`.

```
== parse ==
config <config> api <api>
rc <int>
<field>\t<field>\t<field>[\t<lineno>]
```

`<int>` is the C return code: `0` success, `>0` first error line, `-1` open failure, `-2` allocation failure. The handler used by the drivers always returns `1` (it does not reproduce the unit test's `user=parse_error` rejection).

One call line per `ini_handler` invocation, in order. Three fields, or four when that config sets `INI_HANDLER_LINENO`:

1. section (never NULL; may be empty)
2. name (`null` when the pointer is NULL)
3. value (`null` when the pointer is NULL)
4. lineno, printed only for `handler_lineno`, as a decimal integer with no padding

String fields are quoted. `null` is the unquoted token for a NULL pointer (distinct from `""`). Bytes inside quotes are raw except:

| Byte | Encoding |
|------|----------|
| `\` or `"` | backslash + the byte |
| LF | `\n` |
| CR | `\r` |
| TAB | `\t` |
| other bytes `< 0x20` or `0x7f` | `\x` + two lowercase hex digits |

UTF-8 and other bytes `>= 0x80` are copied unchanged. Lines end with `\n`. No spaces around tabs.

## Configs

Header defaults unless listed. Order is fixed.

| config | macros |
|--------|--------|
| `default` | header defaults (`INI_MAX_LINE=200`, multiline, inline comments, stack, …) |
| `max_line_20` | `INI_MAX_LINE=20` |
| `no_multiline` | `INI_ALLOW_MULTILINE=0` |
| `no_inline_comments` | `INI_ALLOW_INLINE_COMMENTS=0` |
| `stop_on_first_error` | `INI_STOP_ON_FIRST_ERROR=1` |
| `handler_lineno` | `INI_HANDLER_LINENO=1` |
| `heap` | `INI_USE_STACK=0` |
| `heap_max_line` | `INI_USE_STACK=0` `INI_MAX_LINE=20` `INI_INITIAL_ALLOC=20` |
| `heap_realloc` | `INI_USE_STACK=0` `INI_ALLOW_REALLOC=1` `INI_INITIAL_ALLOC=5` |
| `heap_realloc_max_line` | `INI_USE_STACK=0` `INI_MAX_LINE=20` `INI_ALLOW_REALLOC=1` `INI_INITIAL_ALLOC=5` |
| `call_handler_on_new_section` | `INI_CALL_HANDLER_ON_NEW_SECTION=1` |
| `allow_no_value` | `INI_ALLOW_NO_VALUE=1` |
| `custom_alloc` | `INI_CUSTOM_ALLOCATOR=1` `INI_USE_STACK=0` `INI_ALLOW_REALLOC=1` `INI_INITIAL_ALLOC=12` |

`tests/unittest.sh` names `string` and `heap_string` are the `string` api of `max_line_20` and `heap_max_line`, not extra configs. Heap and `custom_alloc` do not change the callback text when allocation succeeds; they are still separate oracle binaries so the line-buffer policy is the one `ini.c` was compiled with.

## Exit status

| Code | Meaning |
|------|---------|
| 0 | Report written |
| 2 | Bad arguments or unknown section |
| other | Oracle or driver failed before a report |
