//! Same stdout as `tools/hook-trace.c`.

#![forbid(unsafe_code)]

use std::cell::RefCell;
use std::io::{self, Write};
use std::process::exit;

use inih_core::{parse_bytes, parse_cstr, return_code, AllocOp, AllocSink, Config, Error};

struct LogAlloc<'a> {
    fail_n: u32,
    n: u32,
    log: &'a RefCell<Vec<u8>>,
}

impl AllocSink for LogAlloc<'_> {
    fn observe(&mut self, op: AllocOp, size: Option<usize>) -> bool {
        let size = size.unwrap_or(0);
        let mut log = self.log.borrow_mut();
        match op {
            AllocOp::Malloc => {
                let _ = writeln!(log, "M {size}");
                self.n += 1;
            }
            AllocOp::Realloc => {
                let _ = writeln!(log, "R {size}");
                self.n += 1;
            }
            AllocOp::Free => {
                let _ = writeln!(log, "F {size}");
            }
        }
        self.fail_n == 0 || op == AllocOp::Free || self.n != self.fail_n
    }
}

fn quote(out: &mut Vec<u8>, field: Option<&[u8]>) {
    let Some(bytes) = field else {
        out.extend_from_slice(b"null");
        return;
    };
    out.push(b'"');
    for &c in bytes {
        match c {
            b'\\' | b'"' => {
                out.push(b'\\');
                out.push(c);
            }
            b'\n' => out.extend_from_slice(b"\\n"),
            b'\r' => out.extend_from_slice(b"\\r"),
            b'\t' => out.extend_from_slice(b"\\t"),
            0x00..=0x1f | 0x7f => {
                let _ = write!(out, "\\x{c:02x}");
            }
            _ => out.push(c),
        }
    }
    out.push(b'"');
}

fn run_parse(
    cfg: &Config,
    fail_n: u32,
    data: &[u8],
    as_cstr: bool,
) -> (Vec<u8>, Result<(), Error>) {
    let log = RefCell::new(Vec::new());
    let mut alloc = LogAlloc {
        fail_n,
        n: 0,
        log: &log,
    };
    let mut handler = |section: &[u8], name: Option<&[u8]>, value: Option<&[u8]>, _line: i32| {
        let mut line = Vec::new();
        line.extend_from_slice(b"H\t");
        quote(&mut line, Some(section));
        line.push(b'\t');
        quote(&mut line, name);
        line.push(b'\t');
        quote(&mut line, value);
        line.push(b'\n');
        log.borrow_mut().extend_from_slice(&line);
        true
    };
    let result = if as_cstr {
        parse_cstr(data, cfg, &mut handler, &mut alloc)
    } else {
        parse_bytes(data, cfg, &mut handler, &mut alloc)
    };
    (log.into_inner(), result)
}

fn run_labeled(
    out: &mut Vec<u8>,
    cfg: &Config,
    fail_n: u32,
    api: &str,
    label: &str,
    data: &[u8],
    as_cstr: bool,
) {
    let _ = writeln!(out, "api {api} {label}");
    let (trace, result) = run_parse(cfg, fail_n, data, as_cstr);
    out.extend_from_slice(&trace);
    let _ = writeln!(out, "rc {}", return_code(&result));
}

fn run_string_pair(out: &mut Vec<u8>, cfg: &Config, fail_n: u32, label: &str, text: &str) {
    let bytes = text.as_bytes();
    run_labeled(out, cfg, fail_n, "string", label, bytes, true);
    run_labeled(out, cfg, fail_n, "length", label, bytes, false);
}

fn run_missing(out: &mut Vec<u8>, api: &str, path: &str) {
    let _ = writeln!(out, "api {api} {path}");
    out.extend_from_slice(b"rc -1\n");
}

fn run_file(out: &mut Vec<u8>, cfg: &Config, fail_n: u32, path: &str) {
    match std::fs::read(path) {
        Ok(bytes) => {
            for api in ["file", "fileptr", "stream"] {
                run_labeled(out, cfg, fail_n, api, path, &bytes, false);
            }
            run_labeled(out, cfg, fail_n, "string", path, &bytes, true);
            run_labeled(out, cfg, fail_n, "length", path, &bytes, false);
        }
        Err(_) => {
            for api in ["file", "fileptr", "stream", "string", "length"] {
                run_missing(out, api, path);
            }
        }
    }
}

fn main() {
    let mut config_name = String::from("default");
    let mut fail_n: u32 = 0;
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        if arg == "--config" {
            let Some(name) = args.next() else {
                eprintln!("missing --config");
                exit(2);
            };
            config_name = name;
        } else if arg == "--fail" {
            let Some(n) = args.next() else {
                eprintln!("missing --fail");
                exit(2);
            };
            let Ok(n) = n.parse::<u32>() else {
                eprintln!("bad --fail");
                exit(2);
            };
            fail_n = n;
        } else {
            eprintln!("usage: inih-hook-trace --config NAME --fail N");
            exit(2);
        }
    }
    let Some((_, cfg)) = inih_core::configs()
        .into_iter()
        .find(|(n, _)| *n == config_name)
    else {
        eprintln!("unknown config");
        exit(2);
    };
    let mut out = Vec::new();
    let _ = writeln!(out, "config {config_name}");
    let _ = writeln!(out, "fail {fail_n}");
    run_string_pair(&mut out, &cfg, fail_n, "empty", "");
    run_string_pair(
        &mut out,
        &cfg,
        fail_n,
        "basic",
        "[section]\nfoo = bar\nbazz = buzz quxx",
    );
    run_string_pair(&mut out, &cfg, fail_n, "sections", "[a]\nx=1\n\n[b]\ny=2\n");
    run_file(&mut out, &cfg, fail_n, "tests/normal.ini");
    run_file(&mut out, &cfg, fail_n, "tests/bad_comment.ini");
    run_file(&mut out, &cfg, fail_n, "tests/bom.ini");
    let mut stdout = io::stdout().lock();
    if stdout.write_all(&out).is_err() {
        exit(1);
    }
}
