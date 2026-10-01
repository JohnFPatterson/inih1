//! Shared dumper for the ported `tests/unittest*.c` suites.
//! Each suite crate uses only some of these helpers.
#![allow(dead_code)]

use std::cell::RefCell;
use std::io::Write;
use std::path::PathBuf;

use inih_core::{parse_bytes, parse_path, return_code, AllocOp, AllocSink, Config, NopAlloc};

pub fn tests_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../tests")
}

pub fn preset(name: &str) -> Config {
    inih_core::configs()
        .into_iter()
        .find(|(n, _)| *n == name)
        .unwrap_or_else(|| panic!("missing preset {name}"))
        .1
}

fn assert_utf8(got: &[u8], expected: &[u8], label: &str) {
    if got != expected {
        panic!(
            "{label}\n--- expected ---\n{}\n--- got ---\n{}",
            String::from_utf8_lossy(expected),
            String::from_utf8_lossy(got)
        );
    }
}

fn section_changed(prev: &[u8], section: &[u8]) -> bool {
    prev != section
}

/// `dumper` from `tests/unittest.c`.
fn dump_unittest(
    out: &mut Vec<u8>,
    prev: &mut Vec<u8>,
    section: &[u8],
    name: Option<&[u8]>,
    value: Option<&[u8]>,
    lineno: Option<i32>,
) -> bool {
    if name.is_none() || section_changed(prev, section) {
        out.extend_from_slice(b"... [");
        out.extend_from_slice(section);
        out.extend_from_slice(b"]\n");
        prev.clear();
        prev.extend_from_slice(section);
    }
    let Some(name) = name else {
        return true;
    };
    out.extend_from_slice(b"... ");
    out.extend_from_slice(name);
    if let Some(value) = value {
        out.push(b'=');
        out.extend_from_slice(value);
    }
    if let Some(line) = lineno {
        let _ = writeln!(out, ";  line {line}");
    } else {
        out.extend_from_slice(b";\n");
    }
    if value.is_none() {
        return true;
    }
    !(name == b"user" && value == Some(b"parse_error"))
}

const FILES: &[&str] = &[
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
];

pub fn run_file_suite(cfg: &Config, baseline: &str) {
    let dir = tests_dir();
    let mut out = Vec::new();
    let mut shown_user = 0i32;
    let mut u = 100i32;
    let with_lineno = cfg.handler_lineno;
    for name in FILES {
        let mut prev = Vec::new();
        let path = dir.join(name);
        let mut handler = |section: &[u8], key: Option<&[u8]>, value: Option<&[u8]>, line: i32| {
            shown_user = u;
            dump_unittest(
                &mut out,
                &mut prev,
                section,
                key,
                value,
                if with_lineno { Some(line) } else { None },
            )
        };
        let mut alloc = NopAlloc;
        let result = parse_path(&path, cfg, &mut handler, &mut alloc);
        let _ = writeln!(out, "{name}: e={} user={shown_user}", return_code(&result));
        u += 1;
    }
    let expected =
        std::fs::read(dir.join(baseline)).unwrap_or_else(|e| panic!("read {baseline}: {e}"));
    assert_utf8(&out, &expected, baseline);
}

const STRINGS: &[(&str, &str)] = &[
    ("empty string", ""),
    ("basic", "[section]\nfoo = bar\nbazz = buzz quxx"),
    ("crlf", "[section]\r\nhello = world\r\nforty_two = 42\r\n"),
    ("long line", "[sec]\nfoo = 01234567890123456789\nbar=4321\n"),
    ("long continued", "[sec]\nfoo = 0123456789012bix=1234\n"),
    ("error", "[s]\na=1\nb\nc=3"),
];

fn dump_string(out: &mut Vec<u8>, prev: &mut Vec<u8>, section: &[u8], name: &[u8], value: &[u8]) {
    if section_changed(prev, section) {
        out.extend_from_slice(b"... [");
        out.extend_from_slice(section);
        out.extend_from_slice(b"]\n");
        prev.clear();
        prev.extend_from_slice(section);
    }
    out.extend_from_slice(b"... ");
    out.extend_from_slice(name);
    out.push(b'=');
    out.extend_from_slice(value);
    out.extend_from_slice(b";\n");
}

pub fn run_string_suite(cfg: &Config, baseline: &str) {
    let mut out = Vec::new();
    let mut shown_user = 0i32;
    let mut u = 100i32;
    for (label, text) in STRINGS {
        let mut prev = Vec::new();
        let mut handler =
            |section: &[u8], name: Option<&[u8]>, value: Option<&[u8]>, _line: i32| {
                shown_user = u;
                let name = name.unwrap_or(b"");
                let value = value.unwrap_or(b"");
                dump_string(&mut out, &mut prev, section, name, value);
                true
            };
        let mut alloc = NopAlloc;
        let result = parse_bytes(text.as_bytes(), cfg, &mut handler, &mut alloc);
        let _ = writeln!(out, "{label}: e={} user={shown_user}", return_code(&result));
        u += 1;
    }
    let expected = std::fs::read(tests_dir().join(baseline))
        .unwrap_or_else(|e| panic!("read {baseline}: {e}"));
    assert_utf8(&out, &expected, baseline);
}

struct PrintAlloc<'a> {
    out: &'a RefCell<Vec<u8>>,
}

impl AllocSink for PrintAlloc<'_> {
    fn observe(&mut self, op: AllocOp, size: Option<usize>) -> bool {
        let mut out = self.out.borrow_mut();
        match op {
            AllocOp::Malloc => {
                let _ = writeln!(out, "ini_malloc({})", size.unwrap_or(0));
            }
            AllocOp::Realloc => {
                let _ = writeln!(out, "ini_realloc({})", size.unwrap_or(0));
            }
            AllocOp::Free => out.extend_from_slice(b"ini_free()\n"),
        }
        true
    }
}

pub fn run_alloc_suite(baseline: &str) {
    let cfg = preset("custom_alloc");
    let text = "[section]\nfoo = bar\nbazz = buzz quxx";
    let out = RefCell::new(Vec::new());
    let mut prev = Vec::new();
    let mut handler = |section: &[u8], name: Option<&[u8]>, value: Option<&[u8]>, _line: i32| {
        let mut buf = out.borrow_mut();
        let name = name.unwrap_or(b"");
        let value = value.unwrap_or(b"");
        dump_string(&mut buf, &mut prev, section, name, value);
        true
    };
    let mut alloc = PrintAlloc { out: &out };
    let result = parse_bytes(text.as_bytes(), &cfg, &mut handler, &mut alloc);
    {
        let mut buf = out.borrow_mut();
        let _ = writeln!(buf, "basic: e={}", return_code(&result));
    }
    let got = out.borrow().clone();
    let expected = std::fs::read(tests_dir().join(baseline))
        .unwrap_or_else(|e| panic!("read {baseline}: {e}"));
    assert_utf8(&got, &expected, baseline);
}
