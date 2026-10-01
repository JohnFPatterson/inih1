//! Line parser. Control flow follows `ini_parse_stream` in `ini.c`.

use std::path::Path;

use crate::Config;

/// C return codes other than success, as a [`Result`] error.
///
/// [`Error::Parse`] is a positive line number: the handler has already been
/// called for every pair `ini.c` would have reported. [`Error::Open`] is `-1`.
/// [`Error::Alloc`] is `-2`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Error {
    Parse { line: i32 },
    Open,
    Alloc,
}

/// `ini.c` return value: `0`, a positive line, `-1`, or `-2`.
pub fn return_code(result: &Result<(), Error>) -> i32 {
    match result {
        Ok(()) => 0,
        Err(Error::Parse { line }) => *line,
        Err(Error::Open) => -1,
        Err(Error::Alloc) => -2,
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AllocOp {
    Malloc,
    Realloc,
    Free,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AllocEvent {
    pub op: AllocOp,
    pub size: Option<usize>,
}

/// Observes the heap line buffer. `false` fails that `malloc`/`realloc`.
///
/// Stack configs never call this. Free's return value is ignored.
pub trait AllocSink {
    fn observe(&mut self, op: AllocOp, size: Option<usize>) -> bool;
}

/// Allocations always succeed and are not recorded.
pub struct NopAlloc;

impl AllocSink for NopAlloc {
    fn observe(&mut self, _op: AllocOp, _size: Option<usize>) -> bool {
        true
    }
}

/// Records logical `malloc` / `realloc` / `free` and can fail the Nth allocation.
///
/// `fail_on` is 1-based over `malloc` and `realloc` only. The failing call is
/// still recorded.
#[derive(Clone, Debug, Default)]
pub struct RecordingAlloc {
    pub fail_on: Option<u32>,
    pub events: Vec<AllocEvent>,
    n: u32,
}

impl AllocSink for RecordingAlloc {
    fn observe(&mut self, op: AllocOp, size: Option<usize>) -> bool {
        self.events.push(AllocEvent { op, size });
        match op {
            AllocOp::Free => true,
            AllocOp::Malloc | AllocOp::Realloc => {
                self.n += 1;
                self.fail_on != Some(self.n)
            }
        }
    }
}

/// `fgets`-style reader. `dest.len()` is the C `num` argument.
///
/// Write a NUL-terminated string of at most `dest.len() - 1` bytes.
/// Return false when no bytes are available.
pub trait IniRead {
    fn read(&mut self, dest: &mut [u8]) -> bool;
}

/// Memory reader with the same stop conditions as `ini_reader_string`.
pub struct SliceReader<'a> {
    data: &'a [u8],
    pos: usize,
}

impl<'a> SliceReader<'a> {
    pub fn new(data: &'a [u8]) -> Self {
        Self { data, pos: 0 }
    }
}

impl IniRead for SliceReader<'_> {
    fn read(&mut self, dest: &mut [u8]) -> bool {
        if dest.len() < 2 || self.pos >= self.data.len() {
            return false;
        }
        let max = dest.len() - 1;
        let mut n = 0;
        while n < max && self.pos < self.data.len() {
            let c = self.data[self.pos];
            self.pos += 1;
            dest[n] = c;
            n += 1;
            if c == b'\n' {
                break;
            }
        }
        dest[n] = 0;
        true
    }
}

/// `section`, optional `name`, optional `value`, `lineno` → nonzero on success.
pub type Handler<'a> = dyn FnMut(&[u8], Option<&[u8]>, Option<&[u8]>, i32) -> bool + 'a;

/// Parse `data` with `ini_parse_string_length` semantics (the full slice).
pub fn parse_bytes(
    data: &[u8],
    cfg: &Config,
    handler: &mut Handler<'_>,
    alloc: &mut dyn AllocSink,
) -> Result<(), Error> {
    let mut reader = SliceReader::new(data);
    parse_reader(&mut reader, cfg, handler, alloc)
}

/// Parse with `ini_parse_string` semantics: stop at the first NUL.
pub fn parse_cstr(
    data: &[u8],
    cfg: &Config,
    handler: &mut Handler<'_>,
    alloc: &mut dyn AllocSink,
) -> Result<(), Error> {
    let n = data.iter().position(|&b| b == 0).unwrap_or(data.len());
    parse_bytes(&data[..n], cfg, handler, alloc)
}

/// Parse a filesystem path the way `ini_parse` does (`-1` if it cannot be read).
pub fn parse_path(
    path: &Path,
    cfg: &Config,
    handler: &mut Handler<'_>,
    alloc: &mut dyn AllocSink,
) -> Result<(), Error> {
    match std::fs::read(path) {
        Ok(data) => parse_bytes(&data, cfg, handler, alloc),
        Err(_) => Err(Error::Open),
    }
}

pub fn parse_reader(
    reader: &mut dyn IniRead,
    cfg: &Config,
    handler: &mut Handler<'_>,
    alloc: &mut dyn AllocSink,
) -> Result<(), Error> {
    if cfg.max_line < 2
        || cfg.max_section < 1
        || cfg.max_name < 1
        || (!cfg.use_stack && cfg.initial_alloc < 2)
    {
        return Err(Error::Alloc);
    }

    let mut heap_live = false;
    let mut cap = if cfg.use_stack {
        cfg.max_line
    } else {
        if !alloc.observe(AllocOp::Malloc, Some(cfg.initial_alloc)) {
            return Err(Error::Alloc);
        }
        heap_live = true;
        cfg.initial_alloc
    };
    let mut line = vec![0u8; cap];
    let mut section = vec![0u8; cfg.max_section];
    let mut prev_name = vec![0u8; cfg.max_name];
    let mut error: i32 = 0;
    let mut lineno: i32 = 0;

    loop {
        if !reader.read(&mut line[..cap]) {
            break;
        }
        let mut offset = c_strlen(&line);
        if cfg.allow_realloc && !cfg.use_stack {
            while cap < cfg.max_line && offset == cap - 1 && line[offset - 1] != b'\n' {
                let mut new_cap = cap.saturating_mul(2);
                if new_cap > cfg.max_line {
                    new_cap = cfg.max_line;
                }
                if new_cap <= cap {
                    break;
                }
                if !alloc.observe(AllocOp::Realloc, Some(new_cap)) {
                    let _ = alloc.observe(AllocOp::Free, Some(cap));
                    return Err(Error::Alloc);
                }
                line.resize(new_cap, 0);
                cap = new_cap;
                let num = cap - offset;
                if !reader.read(&mut line[offset..offset + num]) {
                    break;
                }
                offset += c_strlen(&line[offset..]);
            }
        }

        lineno += 1;
        if offset == cap - 1 && line[offset - 1] != b'\n' {
            let mut abyss = [0u8; 16];
            loop {
                if !reader.read(&mut abyss) {
                    break;
                }
                if error == 0 {
                    error = lineno;
                }
                let abyss_len = c_strlen(&abyss);
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
        rstrip(&mut line, start, offset);
        start = lskip(&line, start);

        if is_comment_prefix(&cfg.start_comment_prefixes, line[start]) {
            // Start-of-line comment, including a blank line (*start == NUL).
        } else if cfg.allow_multiline && prev_name[0] != 0 && line[start] != 0 && start > 0 {
            if cfg.allow_inline_comments {
                let end = find_chars_or_comment(&line, start, None, cfg);
                line[end] = 0;
                rstrip(&mut line, start, end);
            }
            let section_b = cstr(&section);
            let name_b = cstr(&prev_name);
            let value_b = cstr_at(&line, start);
            if !handler(&section_b, Some(&name_b), Some(&value_b), lineno) && error == 0 {
                error = lineno;
            }
        } else if line[start] == b'[' {
            let end = find_chars_or_comment(&line, start + 1, Some(b"]"), cfg);
            if line[end] == b']' {
                line[end] = 0;
                strncpy0(&mut section, &line, start + 1);
                if cfg.allow_multiline {
                    prev_name[0] = 0;
                }
                if cfg.call_handler_on_new_section {
                    let section_b = cstr(&section);
                    if !handler(&section_b, None, None, lineno) && error == 0 {
                        error = lineno;
                    }
                }
            } else if error == 0 {
                error = lineno;
            }
        } else if line[start] != 0 {
            let end = find_chars_or_comment(&line, start, Some(b"=:"), cfg);
            if line[end] == b'=' || line[end] == b':' {
                line[end] = 0;
                rstrip(&mut line, start, end);
                let mut value_at = end + 1;
                let mut value_end = end;
                if cfg.allow_inline_comments {
                    let vend = find_chars_or_comment(&line, value_at, None, cfg);
                    line[vend] = 0;
                    value_end = vend;
                }
                value_at = lskip(&line, value_at);
                rstrip(&mut line, value_at, value_end);
                if cfg.allow_multiline {
                    strncpy0(&mut prev_name, &line, start);
                }
                let section_b = cstr(&section);
                let name_b = cstr_at(&line, start);
                let value_b = cstr_at(&line, value_at);
                if !handler(&section_b, Some(&name_b), Some(&value_b), lineno) && error == 0 {
                    error = lineno;
                }
            } else if cfg.allow_no_value {
                line[end] = 0;
                rstrip(&mut line, start, end);
                let section_b = cstr(&section);
                let name_b = cstr_at(&line, start);
                if !handler(&section_b, Some(&name_b), None, lineno) && error == 0 {
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

    if heap_live {
        let _ = alloc.observe(AllocOp::Free, Some(cap));
    }
    if error != 0 {
        Err(Error::Parse { line: error })
    } else {
        Ok(())
    }
}

fn is_space(c: u8) -> bool {
    matches!(c, b' ' | b'\t' | b'\n' | b'\r' | 0x0b | 0x0c)
}

/// `strchr(prefixes, c)` is true for NUL because it finds the terminator.
fn is_comment_prefix(prefixes: &str, c: u8) -> bool {
    c == 0 || prefixes.as_bytes().contains(&c)
}

fn c_strlen(buf: &[u8]) -> usize {
    buf.iter().position(|&b| b == 0).unwrap_or(buf.len())
}

fn cstr(buf: &[u8]) -> Vec<u8> {
    buf[..c_strlen(buf)].to_vec()
}

fn cstr_at(buf: &[u8], at: usize) -> Vec<u8> {
    cstr(&buf[at..])
}

fn rstrip(line: &mut [u8], start: usize, mut end: usize) {
    while end > start {
        let prev = end - 1;
        if is_space(line[prev]) {
            line[prev] = 0;
            end = prev;
        } else {
            break;
        }
    }
}

fn lskip(line: &[u8], mut i: usize) -> usize {
    while i < line.len() && line[i] != 0 && is_space(line[i]) {
        i += 1;
    }
    i
}

fn find_chars_or_comment(line: &[u8], mut s: usize, chars: Option<&[u8]>, cfg: &Config) -> usize {
    let prefixes = cfg.inline_comment_prefixes.as_bytes();
    if cfg.allow_inline_comments {
        let mut was_space = false;
        while s < line.len()
            && line[s] != 0
            && !chars.is_some_and(|c| c.contains(&line[s]))
            && !(was_space && prefixes.contains(&line[s]))
        {
            was_space = is_space(line[s]);
            s += 1;
        }
    } else {
        while s < line.len() && line[s] != 0 && !chars.is_some_and(|c| c.contains(&line[s])) {
            s += 1;
        }
    }
    s
}

fn strncpy0(dest: &mut [u8], src: &[u8], src_at: usize) {
    let max = dest.len().saturating_sub(1);
    let mut i = 0;
    while i < max && src_at + i < src.len() && src[src_at + i] != 0 {
        dest[i] = src[src_at + i];
        i += 1;
    }
    if !dest.is_empty() {
        dest[i] = 0;
    }
}
