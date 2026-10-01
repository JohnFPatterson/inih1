# Oracle driver format

Both `./build/oracle` and `./target/release/rust-driver` print this format.
The parity gate compares stdout and the exit status byte for byte.

The library has no printer. A run is a SAX trace of `ini_handler` calls, optional
reader and allocator hooks, and the `ini_parse*` return code.

## Invocation

```
driver [--sections name[,name...]] [--alloc-fail N] <fixture>
```

- `<fixture>` is a path. The gate substitutes the fixture path for `{input}`.
- `--sections` chooses blocks. Default: `file,string,alloc`.
  Names: `file`, `string`, `alloc`, `allocfile`.
- `--alloc-fail N` (`N >= 0`) makes the custom-allocator configuration fail the
  Nth `malloc`/`realloc` in each parse (0-based). `free` does not count.
  Omitted means allocations succeed. The counter resets at every parse call.
- Exit `0` after a run, including parse errors (those are `R` lines).
  Exit `2` on usage errors. Drivers do not crash on a missing or malformed file.

`LC_ALL` is set to `C` by the driver (`setlocale`).

## Blocks

| Section | What it runs |
|---|---|
| `file` | Every `tests/unittest.sh` file configuration, on `<fixture>`, through `ini_parse`, `ini_parse_file`, `ini_parse_stream`, `ini_parse_string`, and `ini_parse_string_length` |
| `string` | `string` and `heap_string` configurations, on the hardcoded `tests/unittest_string.c` cases, through `ini_parse_string` only |
| `alloc` | `alloc` configuration, on the hardcoded `tests/unittest_alloc.c` string, through `ini_parse_string` |
| `allocfile` | `alloc` configuration only, on `<fixture>`, through all five entry points. Used by hook-trace, not the default gate run |

File configurations, in order: `multi` (default `ini.h` macros), `multi_max_line`,
`single`, `disallow_inline_comments`, `stop_on_first_error`, `handler_lineno`,
`heap`, `heap_max_line`, `heap_realloc`, `heap_realloc_max_line`,
`call_handler_on_new_section`, `allow_no_value`, `alloc`.

`multi` is the default ABI (`ini_parse*` from `ini.h` / `inih-ffi`). The other
configurations are separate translation units in the C oracle. `handler_lineno`
is the only one whose handler receives a line number.

## Lines

UTF-8, one record per line, fields separated by a single tab (U+0009).

| Tag | Fields |
|---|---|
| `S` | section name |
| `K` | configuration name |
| `C` | configuration, case name (string and alloc blocks) |
| `D` | configuration, api, `num` (reader buffer size), data or `\N` if the reader returned NULL |
| `H` | configuration, api, lineno or `-`, handler return (`0` or `1`), section, name, value |
| `A` | `malloc` or `realloc`, size in bytes; or `free` alone |
| `R` | configuration, api, `ini_parse*` code (`0`, `>0` line, `-1` open, `-2` alloc), user id or `-` |

APIs: `parse`, `file`, `stream`, `string`, `length`.

The handler returns `0` only when `name` is `user` and `value` is `parse_error`
(the `tests/unittest.c` dumper). Otherwise it returns `1`. The user id is `100`
when the handler ran, and `-` when it did not. A missing file is code `-1`
and does not invent a user id.

`lineno` is the integer passed to the handler when the configuration was built
with `INI_HANDLER_LINENO=1`. Every other configuration prints `-` because that
callback has no line-number parameter.

Null `name` or `value` (new-section callback, or `INI_ALLOW_NO_VALUE`) is the
two-character field `\N`. An empty string is an empty field. Section is never null.

`D` lines are emitted only for the `stream` API, from the `ini_reader` callback,
including the 16-byte discard buffer used when a line exceeds the limit.
`A` lines are emitted only by the `alloc` configuration (`INI_CUSTOM_ALLOCATOR`),
in the same order as `ini_malloc` / `ini_realloc` / `ini_free`.

## Escaping

Text fields (`D` data, section, name, value) escape one byte at a time:

| Byte | Output |
|---|---|
| `\` | `\\` |
| newline | `\n` |
| CR | `\r` |
| tab | `\t` |
| other bytes `< 0x20` or `0x7F` | `\x` plus two lowercase hex digits |
| anything else | the raw byte |

`\N` is only produced for a null pointer, never for a value whose bytes are `\N`
(that value is escaped as `\\N`).
