//! Differential driver. Format: `tools/DRIVER_FORMAT.md`.

#![forbid(unsafe_code)]

use std::io::{self, Write};
use std::process::exit;

use inih_core::{parse_bytes, parse_cstr, return_code, Config, NopAlloc};

fn main() {
    let mut sections: Option<Vec<String>> = None;
    let mut fixture: Option<String> = None;
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        if arg == "--sections" {
            let Some(list) = args.next() else {
                eprintln!("missing --sections value");
                exit(2);
            };
            let mut names = Vec::new();
            for part in list.split(',') {
                if part.is_empty() {
                    continue;
                }
                if part != "parse" {
                    eprintln!("unknown section");
                    exit(2);
                }
                names.push(part.to_string());
            }
            sections = Some(names);
        } else if fixture.is_none() {
            fixture = Some(arg);
        } else {
            eprintln!("usage: inih-driver [--sections parse] <fixture>");
            exit(2);
        }
    }
    let Some(fixture) = fixture else {
        eprintln!("usage: inih-driver [--sections parse] <fixture>");
        exit(2);
    };
    let show = match &sections {
        None => true,
        Some(names) => names.iter().any(|n| n == "parse"),
    };
    if !show {
        return;
    }

    let mut out = Vec::new();
    out.extend_from_slice(b"== parse ==\n");
    let data = std::fs::read(&fixture).ok();
    for (name, cfg) in inih_core::configs() {
        for api in ["file", "fileptr", "stream", "string", "length"] {
            emit(&mut out, name, api, data.as_deref(), &cfg);
        }
    }
    let mut stdout = io::stdout().lock();
    if stdout.write_all(&out).is_err() {
        exit(1);
    }
}

struct Call {
    section: Vec<u8>,
    name: Option<Vec<u8>>,
    value: Option<Vec<u8>>,
    lineno: i32,
}

fn emit(out: &mut Vec<u8>, config: &str, api: &str, data: Option<&[u8]>, cfg: &Config) {
    let _ = writeln!(out, "config {config} api {api}");
    let Some(data) = data else {
        out.extend_from_slice(b"rc -1\n");
        return;
    };
    let mut calls = Vec::new();
    let mut handler = |section: &[u8], name: Option<&[u8]>, value: Option<&[u8]>, lineno: i32| {
        calls.push(Call {
            section: section.to_vec(),
            name: name.map(|s| s.to_vec()),
            value: value.map(|s| s.to_vec()),
            lineno,
        });
        true
    };
    let mut alloc = NopAlloc;
    let result = if api == "string" {
        parse_cstr(data, cfg, &mut handler, &mut alloc)
    } else {
        parse_bytes(data, cfg, &mut handler, &mut alloc)
    };
    let _ = writeln!(out, "rc {}", return_code(&result));
    for call in calls {
        quote(out, Some(&call.section));
        out.push(b'\t');
        quote(out, call.name.as_deref());
        out.push(b'\t');
        quote(out, call.value.as_deref());
        if cfg.handler_lineno {
            let _ = write!(out, "\t{}", call.lineno);
        }
        out.push(b'\n');
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
