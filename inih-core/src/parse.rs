//! Line parser. The control flow follows `ini.c` `ini_parse_stream`.

use std::path::Path;

use crate::Config;

/// Failure codes match `ini_parse`: `Line` is the 1-based line of the first
/// error (parsing may have continued), `Open` is `-1`, `Alloc` is `-2`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Error {
    Line(i32),
    Open,
    Alloc,
}

/// Map an `ini_parse*` code onto [`Error`]. `0` is success.
pub fn classify(code: i32) -> Result<(), Error> {
    match code {
        0 => Ok(()),
        -1 => Err(Error::Open),
        -2 => Err(Error::Alloc),
        n => Err(Error::Line(n)),
    }
}

/// One `ini_handler` call. `name` / `value` are `None` when the C pointer is NULL.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Call<'a> {
    pub section: &'a [u8],
    pub name: Option<&'a [u8]>,
    pub value: Option<&'a [u8]>,
    pub lineno: i32,
}

/// Logical heap-buffer events. Emitted only when `Config::use_stack` is false,
/// in the same order as `ini_malloc` / `ini_realloc` / `ini_free`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AllocOp {
    Malloc(usize),
    Realloc(usize),
    Free,
}

/// `fgets` / `ini_reader_string` on a byte slice.
///
/// Writes a NUL-terminated chunk into `dst` (at most `dst.len() - 1` bytes,
/// stopping after a newline). Returns false when nothing was read.
pub fn read_fgets(src: &[u8], pos: &mut usize, dst: &mut [u8]) -> bool {
    if dst.len() < 2 || *pos >= src.len() {
        return false;
    }
    let max = dst.len() - 1;
    let mut n = 0;
    while n < max && *pos < src.len() {
        let c = src[*pos];
        *pos += 1;
        dst[n] = c;
        n += 1;
        if c == b'\n' {
            break;
        }
    }
    dst[n] = 0;
    true
}

/// Parse `data` the way `ini_parse_string_length` does.
pub fn parse_bytes<H, A>(cfg: &Config, data: &[u8], handler: H, alloc: A) -> i32
where
    H: FnMut(Call<'_>) -> i32,
    A: FnMut(AllocOp) -> bool,
{
    let mut pos = 0;
    parse_reader(cfg, |dst| read_fgets(data, &mut pos, dst), handler, alloc)
}

/// Parse `data` the way `ini_parse_string` does: stop at the first NUL.
pub fn parse_cstr<H, A>(cfg: &Config, data: &[u8], handler: H, alloc: A) -> i32
where
    H: FnMut(Call<'_>) -> i32,
    A: FnMut(AllocOp) -> bool,
{
    let n = data.iter().position(|b| *b == 0).unwrap_or(data.len());
    parse_bytes(cfg, &data[..n], handler, alloc)
}

/// Read a whole file and parse it. A missing or unreadable path returns `-1`.
pub fn parse_path<H, A, P>(cfg: &Config, path: P, handler: H, alloc: A) -> i32
where
    H: FnMut(Call<'_>) -> i32,
    A: FnMut(AllocOp) -> bool,
    P: AsRef<Path>,
{
    match std::fs::read(path) {
        Ok(data) => parse_bytes(cfg, &data, handler, alloc),
        Err(_) => -1,
    }
}

/// Parse via an `ini_reader`-style callback. `read` fills `dst` like `fgets`
/// and returns false on end of input. `alloc` is consulted only for a heap
/// line buffer; returning false is the NULL return from `ini_malloc` /
/// `ini_realloc`.
pub fn parse_reader<R, H, A>(cfg: &Config, mut read: R, mut handler: H, mut alloc: A) -> i32
where
    R: FnMut(&mut [u8]) -> bool,
    H: FnMut(Call<'_>) -> i32,
    A: FnMut(AllocOp) -> bool,
{
    if !cfg.limits_ok() {
        return -2;
    }

    let mut cap = if cfg.use_stack {
        cfg.max_line
    } else {
        cfg.initial_alloc
    };
    let mut line = vec![0u8; cap];
    let mut owned = false;
    if !cfg.use_stack {
        if !alloc(AllocOp::Malloc(cap)) {
            return -2;
        }
        owned = true;
    }

    let mut section = vec![0u8; cfg.max_section];
    let mut prev_name = vec![0u8; cfg.max_name];
    let mut lineno: i32 = 0;
    let mut error: i32 = 0;

    loop {
        if !read(&mut line[..cap]) {
            break;
        }
        let mut offset = cstr_len(&line[..cap]);

        if cfg.allow_realloc && !cfg.use_stack {
            while cap < cfg.max_line && offset == cap - 1 && offset > 0 && line[offset - 1] != b'\n'
            {
                let mut next = cap.saturating_mul(2);
                if next > cfg.max_line {
                    next = cfg.max_line;
                }
                if next <= cap {
                    break;
                }
                if !alloc(AllocOp::Realloc(next)) {
                    if owned {
                        let _ = alloc(AllocOp::Free);
                    }
                    return -2;
                }
                line.resize(next, 0);
                if !read(&mut line[offset..next]) {
                    cap = next;
                    break;
                }
                let added = cstr_len(&line[offset..next]);
                offset += added;
                cap = next;
            }
        }

        let Some(next_line) = lineno.checked_add(1) else {
            if owned {
                let _ = alloc(AllocOp::Free);
            }
            return -2;
        };
        lineno = next_line;

        if offset == cap - 1 && offset > 0 && line[offset - 1] != b'\n' {
            let mut abyss = [0u8; 16];
            while read(&mut abyss) {
                if error == 0 {
                    error = lineno;
                }
                let abyss_len = cstr_len(&abyss);
                if abyss_len > 0 && abyss[abyss_len - 1] == b'\n' {
                    break;
                }
            }
        }

        let mut start = 0usize;
        if cfg.allow_bom
            && lineno == 1
            && line.len() >= 3
            && line[0] == 0xEF
            && line[1] == 0xBB
            && line[2] == 0xBF
        {
            start = 3;
        }
        start = lskip(&line, start);
        rstrip(&mut line, offset, start);

        let lead = line[start];
        if is_comment_start(cfg.start_comment_prefixes, lead) {
            // Blank lines and start-of-line comments. `strchr` on the prefix
            // string is true for NUL, so an empty line lands here.
        } else if cfg.allow_multiline && prev_name[0] != 0 && line[start] != 0 && start > 0 {
            if cfg.allow_inline_comments {
                let end = find_end(&line, start, None, true, cfg.inline_comment_prefixes);
                if end < line.len() {
                    line[end] = 0;
                }
                rstrip(&mut line, end, start);
            }
            let name_len = cstr_len(&prev_name);
            let value_len = cstr_len(&line[start..]);
            let sec_len = cstr_len(&section);
            let rc = handler(Call {
                section: &section[..sec_len],
                name: Some(&prev_name[..name_len]),
                value: Some(&line[start..start + value_len]),
                lineno,
            });
            if rc == 0 && error == 0 {
                error = lineno;
            }
        } else if line[start] == b'[' {
            let content = start + 1;
            let end = find_end(
                &line,
                content,
                Some(b"]"),
                cfg.allow_inline_comments,
                cfg.inline_comment_prefixes,
            );
            if end < line.len() && line[end] == b']' {
                line[end] = 0;
                strncpy0(&mut section, &line[content..]);
                if cfg.allow_multiline {
                    prev_name[0] = 0;
                }
                if cfg.call_handler_on_new_section {
                    let sec_len = cstr_len(&section);
                    let rc = handler(Call {
                        section: &section[..sec_len],
                        name: None,
                        value: None,
                        lineno,
                    });
                    if rc == 0 && error == 0 {
                        error = lineno;
                    }
                }
            } else if error == 0 {
                error = lineno;
            }
        } else if line[start] != 0 {
            let end = find_end(
                &line,
                start,
                Some(b"=:"),
                cfg.allow_inline_comments,
                cfg.inline_comment_prefixes,
            );
            if end < line.len() && (line[end] == b'=' || line[end] == b':') {
                line[end] = 0;
                rstrip(&mut line, end, start);
                let mut value = end + 1;
                let mut value_end = end;
                if cfg.allow_inline_comments {
                    let vend = find_end(&line, value, None, true, cfg.inline_comment_prefixes);
                    if vend < line.len() {
                        line[vend] = 0;
                    }
                    value_end = vend;
                }
                value = lskip(&line, value);
                rstrip(&mut line, value_end, value);
                if cfg.allow_multiline {
                    strncpy0(&mut prev_name, &line[start..]);
                }
                let name_len = cstr_len(&line[start..]);
                let value_len = cstr_len(&line[value..]);
                let sec_len = cstr_len(&section);
                let rc = handler(Call {
                    section: &section[..sec_len],
                    name: Some(&line[start..start + name_len]),
                    value: Some(&line[value..value + value_len]),
                    lineno,
                });
                if rc == 0 && error == 0 {
                    error = lineno;
                }
            } else if cfg.allow_no_value {
                if end < line.len() {
                    line[end] = 0;
                }
                rstrip(&mut line, end, start);
                let name_len = cstr_len(&line[start..]);
                let sec_len = cstr_len(&section);
                let rc = handler(Call {
                    section: &section[..sec_len],
                    name: Some(&line[start..start + name_len]),
                    value: None,
                    lineno,
                });
                if rc == 0 && error == 0 {
                    error = lineno;
                }
            } else if error == 0 {
                error = lineno;
            }
        }

        if cfg.stop_on_first_error && error != 0 {
            break;
        }
    }

    if owned {
        let _ = alloc(AllocOp::Free);
    }
    error
}

fn is_space(b: u8) -> bool {
    matches!(b, b' ' | b'\t' | b'\n' | b'\r' | 0x0b | 0x0c)
}

/// `strchr(prefixes, c)` is true for NUL because the C string ends with one.
fn is_comment_start(prefixes: &[u8], c: u8) -> bool {
    c == 0 || prefixes.contains(&c)
}

fn cstr_len(buf: &[u8]) -> usize {
    buf.iter().position(|b| *b == 0).unwrap_or(buf.len())
}

fn lskip(buf: &[u8], mut i: usize) -> usize {
    while i < buf.len() && buf[i] != 0 && is_space(buf[i]) {
        i += 1;
    }
    i
}

/// `ini_rstrip`: clear trailing spaces before `end` (the index of the NUL).
fn rstrip(line: &mut [u8], mut end: usize, start: usize) {
    while end > start {
        let prev = end - 1;
        if prev >= line.len() || !is_space(line[prev]) {
            break;
        }
        line[prev] = 0;
        end = prev;
    }
}

fn strncpy0(dst: &mut [u8], src: &[u8]) {
    let max = dst.len().saturating_sub(1);
    let mut i = 0;
    while i < max && i < src.len() && src[i] != 0 {
        dst[i] = src[i];
        i += 1;
    }
    if i < dst.len() {
        dst[i] = 0;
    }
}

fn find_end(
    line: &[u8],
    start: usize,
    chars: Option<&[u8]>,
    inline: bool,
    prefixes: &[u8],
) -> usize {
    let mut i = start;
    let mut was_space = false;
    while i < line.len() && line[i] != 0 {
        let c = line[i];
        let hit_char = match chars {
            Some(set) => set.contains(&c),
            None => false,
        };
        let hit_comment = inline && was_space && prefixes.contains(&c);
        if hit_char || hit_comment {
            break;
        }
        was_space = is_space(c);
        i += 1;
    }
    i
}
