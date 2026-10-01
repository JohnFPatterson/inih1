//! Prints the trace described in `tools/DRIVER_FORMAT.md`.

use std::env;
use std::io::{self, Write};
use std::process::ExitCode;

use inih_core::{parse_bytes, parse_cstr, parse_reader, preset, read_fgets, AllocOp, Call};

const FILE_CONFIGS: &[&str] = &[
    "multi",
    "multi_max_line",
    "single",
    "disallow_inline_comments",
    "stop_on_first_error",
    "handler_lineno",
    "heap",
    "heap_max_line",
    "heap_realloc",
    "heap_realloc_max_line",
    "call_handler_on_new_section",
    "allow_no_value",
    "alloc",
];

const STRING_CONFIGS: &[&str] = &["string", "heap_string"];

const STRING_CASES: &[(&str, &str)] = &[
    ("empty string", ""),
    ("basic", "[section]\nfoo = bar\nbazz = buzz quxx"),
    ("crlf", "[section]\r\nhello = world\r\nforty_two = 42\r\n"),
    ("long line", "[sec]\nfoo = 01234567890123456789\nbar=4321\n"),
    ("long continued", "[sec]\nfoo = 0123456789012bix=1234\n"),
    ("error", "[s]\na=1\nb\nc=3"),
];

const ALLOC_CASE: &str = "[section]\nfoo = bar\nbazz = buzz quxx";

struct AllocCtl {
    index: i32,
    fail_after: i32,
    trace: bool,
}

fn main() -> ExitCode {
    let mut sections = "file,string,alloc".to_string();
    let mut fail_after: i32 = -1;
    let mut input: Option<String> = None;
    let mut args = env::args().skip(1);
    while let Some(arg) = args.next() {
        if arg == "--sections" {
            let Some(value) = args.next() else {
                return usage();
            };
            sections = value;
        } else if arg == "--alloc-fail" {
            let Some(value) = args.next() else {
                return usage();
            };
            let Ok(n) = value.parse::<i32>() else {
                return usage();
            };
            if n < 0 {
                return usage();
            }
            fail_after = n;
        } else if arg == "--help" || arg.starts_with('-') || input.is_some() {
            return usage();
        } else {
            input = Some(arg);
        }
    }
    let Some(input) = input else {
        return usage();
    };
    if !sections_known(&sections) {
        return usage();
    }

    let mut alloc = AllocCtl {
        index: 0,
        fail_after,
        trace: false,
    };

    if want(&sections, "file") {
        emit_line("S\tfile");
        for name in FILE_CONFIGS {
            run_file(name, &input, &mut alloc);
        }
    }
    if want(&sections, "allocfile") {
        emit_line("S\tallocfile");
        run_file("alloc", &input, &mut alloc);
    }
    if want(&sections, "string") {
        emit_line("S\tstring");
        for name in STRING_CONFIGS {
            emit_line(&format!("K\t{name}"));
            let Some(cfg) = preset(name) else {
                return ExitCode::from(2);
            };
            for (case, text) in STRING_CASES {
                emit_line(&format!("C\t{name}\t{case}"));
                alloc.trace = false;
                alloc.index = 0;
                let mut called = false;
                let code = parse_cstr(
                    &cfg,
                    text.as_bytes(),
                    |call| handle(name, "string", false, &call, &mut called),
                    |_| true,
                );
                emit_result(name, "string", code, called);
            }
        }
    }
    if want(&sections, "alloc") {
        emit_line("S\talloc");
        emit_line("K\talloc");
        emit_line("C\talloc\tbasic");
        let Some(cfg) = preset("alloc") else {
            return ExitCode::from(2);
        };
        alloc.trace = true;
        alloc.index = 0;
        let mut called = false;
        let code = parse_cstr(
            &cfg,
            ALLOC_CASE.as_bytes(),
            |call| handle("alloc", "string", false, &call, &mut called),
            |op| on_alloc(&mut alloc, op),
        );
        emit_result("alloc", "string", code, called);
    }
    let _ = io::stdout().flush();
    ExitCode::SUCCESS
}

fn run_file(name: &str, path: &str, alloc: &mut AllocCtl) {
    let Some(cfg) = preset(name) else {
        return;
    };
    let show_lineno = name == "handler_lineno";
    alloc.trace = name == "alloc";
    emit_line(&format!("K\t{name}"));
    let bytes = std::fs::read(path).ok();

    alloc.index = 0;
    let mut called = false;
    let code = match &bytes {
        Some(data) => parse_bytes(
            &cfg,
            data,
            |call| handle(name, "parse", show_lineno, &call, &mut called),
            |op| on_alloc(alloc, op),
        ),
        None => -1,
    };
    emit_result(name, "parse", code, called);

    alloc.index = 0;
    called = false;
    let code = match &bytes {
        Some(data) => parse_bytes(
            &cfg,
            data,
            |call| handle(name, "file", show_lineno, &call, &mut called),
            |op| on_alloc(alloc, op),
        ),
        None => -1,
    };
    emit_result(name, "file", code, called);

    alloc.index = 0;
    called = false;
    let code = match &bytes {
        Some(data) => {
            let mut pos = 0usize;
            parse_reader(
                &cfg,
                |dst| {
                    let num = dst.len();
                    let ok = read_fgets(data, &mut pos, dst);
                    emit_read(name, num, if ok { Some(cstr(dst)) } else { None });
                    ok
                },
                |call| handle(name, "stream", show_lineno, &call, &mut called),
                |op| on_alloc(alloc, op),
            )
        }
        None => -1,
    };
    emit_result(name, "stream", code, called);

    alloc.index = 0;
    called = false;
    let code = match &bytes {
        Some(data) => parse_cstr(
            &cfg,
            data,
            |call| handle(name, "string", show_lineno, &call, &mut called),
            |op| on_alloc(alloc, op),
        ),
        None => -1,
    };
    emit_result(name, "string", code, called);

    alloc.index = 0;
    called = false;
    let code = match &bytes {
        Some(data) => parse_bytes(
            &cfg,
            data,
            |call| handle(name, "length", show_lineno, &call, &mut called),
            |op| on_alloc(alloc, op),
        ),
        None => -1,
    };
    emit_result(name, "length", code, called);
}

fn on_alloc(ctl: &mut AllocCtl, op: AllocOp) -> bool {
    if !ctl.trace {
        return true;
    }
    match op {
        AllocOp::Malloc(n) => {
            emit_line(&format!("A\tmalloc\t{n}"));
            let fail = ctl.fail_after >= 0 && ctl.index == ctl.fail_after;
            ctl.index += 1;
            !fail
        }
        AllocOp::Realloc(n) => {
            emit_line(&format!("A\trealloc\t{n}"));
            let fail = ctl.fail_after >= 0 && ctl.index == ctl.fail_after;
            ctl.index += 1;
            !fail
        }
        AllocOp::Free => {
            emit_line("A\tfree");
            true
        }
    }
}

fn handle(config: &str, api: &str, show_lineno: bool, call: &Call<'_>, called: &mut bool) -> i32 {
    *called = true;
    let rc = handler_rc(call.name, call.value);
    let mut line = Vec::new();
    line.extend_from_slice(b"H\t");
    line.extend_from_slice(config.as_bytes());
    line.push(b'\t');
    line.extend_from_slice(api.as_bytes());
    line.push(b'\t');
    if show_lineno {
        line.extend_from_slice(call.lineno.to_string().as_bytes());
    } else {
        line.push(b'-');
    }
    line.push(b'\t');
    line.extend_from_slice(rc.to_string().as_bytes());
    line.push(b'\t');
    escape_into(&mut line, Some(call.section));
    line.push(b'\t');
    escape_into(&mut line, call.name);
    line.push(b'\t');
    escape_into(&mut line, call.value);
    write_bytes(&line);
    rc
}

fn handler_rc(name: Option<&[u8]>, value: Option<&[u8]>) -> i32 {
    if name == Some(b"user") && value == Some(b"parse_error") {
        0
    } else {
        1
    }
}

fn emit_read(config: &str, num: usize, data: Option<&[u8]>) {
    let mut line = Vec::new();
    line.extend_from_slice(b"D\t");
    line.extend_from_slice(config.as_bytes());
    line.extend_from_slice(b"\tstream\t");
    line.extend_from_slice(num.to_string().as_bytes());
    line.push(b'\t');
    escape_into(&mut line, data);
    write_bytes(&line);
}

fn emit_result(config: &str, api: &str, code: i32, called: bool) {
    if called {
        emit_line(&format!("R\t{config}\t{api}\t{code}\t100"));
    } else {
        emit_line(&format!("R\t{config}\t{api}\t{code}\t-"));
    }
}

fn emit_line(text: &str) {
    write_bytes(text.as_bytes());
}

fn write_bytes(bytes: &[u8]) {
    let mut out = io::stdout().lock();
    let _ = out.write_all(bytes);
    let _ = out.write_all(b"\n");
}

fn escape_into(out: &mut Vec<u8>, field: Option<&[u8]>) {
    let Some(bytes) = field else {
        out.extend_from_slice(br"\N");
        return;
    };
    for &c in bytes {
        match c {
            b'\\' => out.extend_from_slice(br"\\"),
            b'\n' => out.extend_from_slice(br"\n"),
            b'\r' => out.extend_from_slice(br"\r"),
            b'\t' => out.extend_from_slice(br"\t"),
            c if c < 0x20 || c == 0x7F => {
                const HEX: &[u8] = b"0123456789abcdef";
                out.extend_from_slice(br"\x");
                out.push(HEX[(c >> 4) as usize]);
                out.push(HEX[(c & 0x0f) as usize]);
            }
            c => out.push(c),
        }
    }
}

fn cstr(buf: &[u8]) -> &[u8] {
    let n = buf.iter().position(|b| *b == 0).unwrap_or(buf.len());
    &buf[..n]
}

fn want(list: &str, name: &str) -> bool {
    list.split(',').any(|part| part == name)
}

fn sections_known(list: &str) -> bool {
    if list.is_empty() {
        return false;
    }
    list.split(',')
        .all(|part| matches!(part, "file" | "string" | "alloc" | "allocfile"))
}

fn usage() -> ExitCode {
    let _ = writeln!(
        io::stderr(),
        "usage: rust-driver [--sections file,string,alloc,allocfile] [--alloc-fail N] <fixture>"
    );
    ExitCode::from(2)
}
