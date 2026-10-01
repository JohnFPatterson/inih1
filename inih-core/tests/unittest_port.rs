//! Case-for-case port of tests/unittest.c, tests/unittest_string.c, and
//! tests/unittest_alloc.c. Each function is one `unittest.sh` configuration.
//! Expected bytes are the checked-in baseline_*.txt files.

use std::cell::RefCell;
use std::path::PathBuf;
use std::rc::Rc;

use inih_core::{parse_cstr, parse_path, preset, AllocOp, Call};

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

fn repo() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("..")
}

fn file_suite(name: &str, show_lineno: bool) -> Result<(), String> {
    let cfg = preset(name).ok_or_else(|| format!("unknown preset {name}"))?;
    let mut out = Vec::new();
    // unittest.c's User starts at 0 and changes only when the handler runs.
    let mut seen_user = 0i32;
    let mut u = 100i32;
    for fname in FILES {
        let mut prev: Vec<u8> = Vec::new();
        let code = parse_path(
            &cfg,
            repo().join("tests").join(fname),
            |call: Call<'_>| {
                seen_user = u;
                dump_file(&mut out, &mut prev, &call, show_lineno);
                dumper_rc(call.name, call.value)
            },
            |_| true,
        );
        out.extend_from_slice(format!("{fname}: e={code} user={seen_user}\n").as_bytes());
        u += 1;
    }
    expect_baseline(&format!("baseline_{name}.txt"), &out)
}

fn string_suite(name: &str) -> Result<(), String> {
    let cfg = preset(name).ok_or_else(|| format!("unknown preset {name}"))?;
    let cases: &[(&str, &str)] = &[
        ("empty string", ""),
        ("basic", "[section]\nfoo = bar\nbazz = buzz quxx"),
        ("crlf", "[section]\r\nhello = world\r\nforty_two = 42\r\n"),
        ("long line", "[sec]\nfoo = 01234567890123456789\nbar=4321\n"),
        ("long continued", "[sec]\nfoo = 0123456789012bix=1234\n"),
        ("error", "[s]\na=1\nb\nc=3"),
    ];
    let mut out = Vec::new();
    let mut seen_user = 0i32;
    let mut u = 100i32;
    for (label, text) in cases {
        let mut prev: Vec<u8> = Vec::new();
        let code = parse_cstr(
            &cfg,
            text.as_bytes(),
            |call: Call<'_>| {
                seen_user = u;
                dump_string(&mut out, &mut prev, &call);
                1
            },
            |_| true,
        );
        out.extend_from_slice(format!("{label}: e={code} user={seen_user}\n").as_bytes());
        u += 1;
    }
    expect_baseline(&format!("baseline_{name}.txt"), &out)
}

fn dump_string(out: &mut Vec<u8>, prev: &mut Vec<u8>, call: &Call<'_>) {
    let name = call.name.unwrap_or(b"");
    let value = call.value.unwrap_or(b"");
    if call.section != prev.as_slice() {
        out.extend_from_slice(b"... [");
        out.extend_from_slice(call.section);
        out.extend_from_slice(b"]\n");
        *prev = call.section.to_vec();
        if prev.len() > 49 {
            prev.truncate(49);
        }
    }
    out.extend_from_slice(b"... ");
    out.extend_from_slice(name);
    out.extend_from_slice(b"=");
    out.extend_from_slice(value);
    out.extend_from_slice(b";\n");
}

fn alloc_suite() -> Result<(), String> {
    let cfg = preset("alloc").ok_or("missing alloc preset")?;
    let buf = Rc::new(RefCell::new(Vec::<u8>::new()));
    let mut prev: Vec<u8> = Vec::new();
    let text = "[section]\nfoo = bar\nbazz = buzz quxx";
    let code = parse_cstr(
        &cfg,
        text.as_bytes(),
        {
            let buf = Rc::clone(&buf);
            move |call: Call<'_>| {
                let mut out = buf.borrow_mut();
                if call.section != prev.as_slice() {
                    out.extend_from_slice(b"... [");
                    out.extend_from_slice(call.section);
                    out.extend_from_slice(b"]\n");
                    prev = call.section.to_vec();
                }
                let name = call.name.unwrap_or(&[]);
                let value = call.value.unwrap_or(&[]);
                out.extend_from_slice(b"... ");
                out.extend_from_slice(name);
                out.extend_from_slice(b"=");
                out.extend_from_slice(value);
                out.extend_from_slice(b";\n");
                1
            }
        },
        {
            let buf = Rc::clone(&buf);
            move |op| {
                let mut out = buf.borrow_mut();
                match op {
                    AllocOp::Malloc(n) => {
                        out.extend_from_slice(format!("ini_malloc({n})\n").as_bytes())
                    }
                    AllocOp::Realloc(n) => {
                        out.extend_from_slice(format!("ini_realloc({n})\n").as_bytes())
                    }
                    AllocOp::Free => out.extend_from_slice(b"ini_free()\n"),
                }
                true
            }
        },
    );
    buf.borrow_mut()
        .extend_from_slice(format!("basic: e={code}\n").as_bytes());
    let got = buf.borrow().clone();
    expect_baseline("baseline_alloc.txt", &got)
}

fn dump_file(out: &mut Vec<u8>, prev: &mut Vec<u8>, call: &Call<'_>, show_lineno: bool) {
    if call.name.is_none() || call.section != prev.as_slice() {
        out.extend_from_slice(b"... [");
        out.extend_from_slice(call.section);
        out.extend_from_slice(b"]\n");
        *prev = call.section.to_vec();
        if prev.len() > 49 {
            prev.truncate(49);
        }
    }
    let Some(name) = call.name else {
        return;
    };
    out.extend_from_slice(b"... ");
    out.extend_from_slice(name);
    if let Some(value) = call.value {
        out.push(b'=');
        out.extend_from_slice(value);
    }
    out.push(b';');
    if show_lineno {
        out.extend_from_slice(format!("  line {}\n", call.lineno).as_bytes());
    } else {
        out.push(b'\n');
    }
}

fn dumper_rc(name: Option<&[u8]>, value: Option<&[u8]>) -> i32 {
    if name == Some(b"user") && value == Some(b"parse_error") {
        0
    } else {
        1
    }
}

fn expect_baseline(name: &str, got: &[u8]) -> Result<(), String> {
    let path = repo().join("tests").join(name);
    let expected = std::fs::read(&path).map_err(|e| format!("read {}: {e}", path.display()))?;
    if got != expected.as_slice() {
        let got_path = std::env::temp_dir().join(format!("inih-got-{name}"));
        std::fs::write(&got_path, got).map_err(|e| e.to_string())?;
        return Err(format!(
            "{name} diverged from {}; wrote {}",
            path.display(),
            got_path.display()
        ));
    }
    Ok(())
}

#[test]
fn unittest_multi() -> Result<(), String> {
    file_suite("multi", false)
}
#[test]
fn unittest_multi_max_line() -> Result<(), String> {
    file_suite("multi_max_line", false)
}
#[test]
fn unittest_single() -> Result<(), String> {
    file_suite("single", false)
}
#[test]
fn unittest_disallow_inline_comments() -> Result<(), String> {
    file_suite("disallow_inline_comments", false)
}
#[test]
fn unittest_stop_on_first_error() -> Result<(), String> {
    file_suite("stop_on_first_error", false)
}
#[test]
fn unittest_handler_lineno() -> Result<(), String> {
    file_suite("handler_lineno", true)
}
#[test]
fn unittest_heap() -> Result<(), String> {
    file_suite("heap", false)
}
#[test]
fn unittest_heap_max_line() -> Result<(), String> {
    file_suite("heap_max_line", false)
}
#[test]
fn unittest_heap_realloc() -> Result<(), String> {
    file_suite("heap_realloc", false)
}
#[test]
fn unittest_heap_realloc_max_line() -> Result<(), String> {
    file_suite("heap_realloc_max_line", false)
}
#[test]
fn unittest_call_handler_on_new_section() -> Result<(), String> {
    file_suite("call_handler_on_new_section", false)
}
#[test]
fn unittest_allow_no_value() -> Result<(), String> {
    file_suite("allow_no_value", false)
}
#[test]
fn unittest_string() -> Result<(), String> {
    string_suite("string")
}
#[test]
fn unittest_heap_string() -> Result<(), String> {
    string_suite("heap_string")
}
#[test]
fn unittest_alloc() -> Result<(), String> {
    alloc_suite()
}

#[test]
fn alloc_fail_stops_like_c() -> Result<(), String> {
    let cfg = preset("alloc").ok_or("alloc")?;
    let text = b"[section]\nfoo = bar\nbazz = buzz quxx";
    let run = |fail_after: i32| -> i32 {
        let index = Rc::new(RefCell::new(0i32));
        parse_cstr(&cfg, text, |_| 1, {
            let index = Rc::clone(&index);
            move |op| match op {
                AllocOp::Free => true,
                AllocOp::Malloc(_) | AllocOp::Realloc(_) => {
                    let mut i = index.borrow_mut();
                    let fail = *i == fail_after;
                    *i += 1;
                    !fail
                }
            }
        })
    };
    if run(0) != -2 {
        return Err("first malloc must return -2".into());
    }
    if run(1) != -2 {
        return Err("realloc failure must return -2".into());
    }
    if run(2) != 0 {
        return Err("two successful allocs must parse".into());
    }
    Ok(())
}
