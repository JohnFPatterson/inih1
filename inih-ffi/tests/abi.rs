//! Pointer-level ABI checks for the default `ini.h` surface.
//!
//! inih does not return interior pointers, alias a caller buffer, or expose a
//! destructor. These tests cover the contracts that do exist: callback
//! pointers live only for the call, `value` may be written, open failure is
//! `-1`, and a handler return of 0 becomes that line number.

use std::ffi::{CStr, CString};
use std::os::raw::{c_char, c_int, c_void};
use std::ptr;

use inih_ffi::{
    ini_parse, ini_parse_file, ini_parse_stream, ini_parse_string, ini_parse_string_length, CFile,
};

extern "C" {
    fn fopen(filename: *const c_char, mode: *const c_char) -> *mut CFile;
    fn fclose(stream: *mut CFile) -> c_int;
}

struct Hit {
    section: Vec<u8>,
    name: Option<Vec<u8>>,
    value: Option<Vec<u8>>,
    wrote_value: bool,
}

fn copy_cstr(p: *const c_char) -> Option<Vec<u8>> {
    if p.is_null() {
        None
    } else {
        // SAFETY: ini.h pointers are NUL-terminated and valid for this callback.
        Some(unsafe { CStr::from_ptr(p) }.to_bytes().to_vec())
    }
}

unsafe extern "C" fn collect(
    user: *mut c_void,
    section: *const c_char,
    name: *const c_char,
    value: *const c_char,
) -> c_int {
    let slot = &mut *(user as *mut Option<Hit>);
    let hit = Hit {
        section: copy_cstr(section).unwrap_or_default(),
        name: copy_cstr(name),
        value: copy_cstr(value),
        wrote_value: false,
    };
    *slot = Some(hit);
    // Header: value may be cast to char* and modified for the duration of the call.
    if !value.is_null() && !name.is_null() {
        let bytes = CStr::from_ptr(value).to_bytes();
        if !bytes.is_empty() {
            let mutable = value as *mut c_char;
            *mutable = b'Z' as c_char;
            (*slot).as_mut().unwrap().wrote_value = true;
        }
    }
    1
}

#[test]
fn string_callbacks_see_copied_fields_not_caller_buffer() {
    let raw = CString::new("[sec]\nname=value\n").unwrap();
    let mut hit: Option<Hit> = None;
    let rc = unsafe {
        ini_parse_string(
            raw.as_ptr(),
            Some(collect),
            &mut hit as *mut Option<Hit> as *mut c_void,
        )
    };
    assert_eq!(rc, 0);
    let hit = hit.expect("handler");
    assert_eq!(hit.section, b"sec");
    assert_eq!(hit.name.as_deref(), Some(&b"name"[..]));
    assert_eq!(hit.value.as_deref(), Some(&b"value"[..]));
    assert!(hit.wrote_value);
    // Mutating the callback's value pointer must not rewrite the caller's buffer.
    assert_eq!(raw.as_bytes(), b"[sec]\nname=value\n");
}

#[test]
fn string_length_honors_explicit_length() {
    let raw = b"a=bX";
    let mut hit: Option<Hit> = None;
    let rc = unsafe {
        ini_parse_string_length(
            raw.as_ptr().cast(),
            3,
            Some(collect),
            &mut hit as *mut Option<Hit> as *mut c_void,
        )
    };
    assert_eq!(rc, 0);
    let hit = hit.expect("handler");
    assert_eq!(hit.name.as_deref(), Some(&b"a"[..]));
    assert_eq!(hit.value.as_deref(), Some(&b"b"[..]));
}

#[test]
fn handler_zero_reports_that_line() {
    unsafe extern "C" fn reject(
        _user: *mut c_void,
        _section: *const c_char,
        _name: *const c_char,
        _value: *const c_char,
    ) -> c_int {
        0
    }
    let raw = CString::new("a=b\n").unwrap();
    let rc = unsafe { ini_parse_string(raw.as_ptr(), Some(reject), ptr::null_mut()) };
    assert_eq!(rc, 1);
}

#[test]
fn missing_file_is_minus_one() {
    let path = CString::new("/no/such/inih-ffi-missing.ini").unwrap();
    let rc = unsafe { ini_parse(path.as_ptr(), Some(collect), ptr::null_mut()) };
    assert_eq!(rc, -1);
}

#[test]
fn parse_file_matches_parse_string() {
    let dir = std::env::temp_dir().join("inih-ffi-abi.ini");
    std::fs::write(&dir, "k=v\n").unwrap();
    let path = CString::new(dir.to_str().unwrap()).unwrap();
    let file = unsafe { fopen(path.as_ptr(), c"r".as_ptr()) };
    assert!(!file.is_null());
    let mut hit: Option<Hit> = None;
    let rc = unsafe {
        ini_parse_file(
            file,
            Some(collect),
            &mut hit as *mut Option<Hit> as *mut c_void,
        )
    };
    unsafe {
        fclose(file);
    }
    let _ = std::fs::remove_file(&dir);
    assert_eq!(rc, 0);
    let hit = hit.expect("file handler");
    assert_eq!(hit.name.as_deref(), Some(&b"k"[..]));
    assert_eq!(hit.value.as_deref(), Some(&b"v"[..]));
}

struct Mem {
    data: &'static [u8],
    pos: usize,
}

unsafe extern "C" fn mem_reader(
    str_buf: *mut c_char,
    num: c_int,
    stream: *mut c_void,
) -> *mut c_char {
    let ctx = &mut *(stream as *mut Mem);
    if ctx.pos >= ctx.data.len() || num < 2 {
        return ptr::null_mut();
    }
    let mut n = 0;
    let max = (num as usize) - 1;
    let out = str_buf as *mut u8;
    while n < max && ctx.pos < ctx.data.len() {
        let c = ctx.data[ctx.pos];
        ctx.pos += 1;
        *out.add(n) = c;
        n += 1;
        if c == b'\n' {
            break;
        }
    }
    *out.add(n) = 0;
    str_buf
}

#[test]
fn parse_stream_uses_caller_reader() {
    let mut mem = Mem {
        data: b"[s]\nx=y\n",
        pos: 0,
    };
    let mut hit: Option<Hit> = None;
    let rc = unsafe {
        ini_parse_stream(
            Some(mem_reader),
            &mut mem as *mut Mem as *mut c_void,
            Some(collect),
            &mut hit as *mut Option<Hit> as *mut c_void,
        )
    };
    assert_eq!(rc, 0);
    let hit = hit.expect("stream");
    assert_eq!(hit.section, b"s");
    assert_eq!(hit.name.as_deref(), Some(&b"x"[..]));
    assert_eq!(hit.value.as_deref(), Some(&b"y"[..]));
}

#[test]
fn empty_length_is_success_without_calls() {
    let mut hit: Option<Hit> = None;
    let rc = unsafe {
        ini_parse_string_length(
            ptr::null(),
            0,
            Some(collect),
            &mut hit as *mut Option<Hit> as *mut c_void,
        )
    };
    assert_eq!(rc, 0);
    assert!(hit.is_none());
}
