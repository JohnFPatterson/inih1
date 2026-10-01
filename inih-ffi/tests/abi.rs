//! Pointer-level checks for the default `ini.h` ABI.
//!
//! inih does not return buffers the caller frees, and it does not store the
//! caller's pointers past the handler call. These tests cover the contracts
//! the SAX driver does not print: null rejection, value mutation during the
//! callback, and agreement of the five entry points on one file.

use std::ffi::{CStr, CString};
use std::os::raw::{c_char, c_int, c_void};

use inih_ffi::{
    ini_parse, ini_parse_file, ini_parse_stream, ini_parse_string, ini_parse_string_length,
};

type Field = Option<Vec<u8>>;
type Event = (Vec<u8>, Field, Field);

struct Sink {
    events: Vec<Event>,
}

extern "C" fn collect(
    user: *mut c_void,
    section: *const c_char,
    name: *const c_char,
    value: *const c_char,
) -> c_int {
    // SAFETY: the library passes NUL-terminated strings that live for this call.
    // Null name or value is part of the ABI but the default configuration does
    // not use it; treat null as a missing field.
    unsafe {
        let sink = &mut *(user as *mut Sink);
        let section = cstr(section);
        let name = if name.is_null() {
            None
        } else {
            Some(cstr(name))
        };
        let value = if value.is_null() {
            None
        } else {
            Some(cstr(value))
        };
        sink.events.push((section, name, value));
    }
    let name_b = name_bytes(name);
    let value_b = name_bytes(value);
    if name_b.as_deref() == Some(b"user".as_slice())
        && value_b.as_deref() == Some(b"parse_error".as_slice())
    {
        0
    } else {
        1
    }
}

fn cstr(p: *const c_char) -> Vec<u8> {
    // SAFETY: caller guarantees p is a non-null NUL-terminated string.
    unsafe { CStr::from_ptr(p).to_bytes().to_vec() }
}

fn fixture(name: &str) -> CString {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("tests")
        .join(name);
    CString::new(path.to_string_lossy().as_bytes()).unwrap_or_else(|_| CString::new("").unwrap())
}

fn name_bytes(p: *const c_char) -> Option<Vec<u8>> {
    if p.is_null() {
        None
    } else {
        Some(cstr(p))
    }
}

extern "C" fn mutate_value(
    _user: *mut c_void,
    _section: *const c_char,
    _name: *const c_char,
    value: *const c_char,
) -> c_int {
    if !value.is_null() {
        // SAFETY: ini.h allows the handler to modify value for this call only.
        // The pointer is writable for the length of the C string.
        unsafe {
            let p = value as *mut c_char;
            if *p != 0 {
                *p = b'Z' as c_char;
            }
        }
    }
    1
}

struct Mem {
    data: &'static [u8],
    pos: usize,
}

extern "C" fn mem_read(buf: *mut c_char, num: c_int, stream: *mut c_void) -> *mut c_char {
    // SAFETY: stream is the Mem we passed, and buf has room for `num` bytes.
    unsafe {
        let mem = &mut *(stream as *mut Mem);
        if num < 2 || mem.pos >= mem.data.len() {
            return std::ptr::null_mut();
        }
        let mut n = 0;
        let max = (num - 1) as usize;
        let dst = std::slice::from_raw_parts_mut(buf as *mut u8, num as usize);
        while n < max && mem.pos < mem.data.len() {
            let c = mem.data[mem.pos];
            mem.pos += 1;
            dst[n] = c;
            n += 1;
            if c == b'\n' {
                break;
            }
        }
        dst[n] = 0;
        buf
    }
}

#[test]
fn null_callbacks_return_error() {
    // CH-001: C asserts or dereferences these. The shim returns -1.
    unsafe {
        assert_eq!(
            ini_parse(std::ptr::null(), Some(collect), std::ptr::null_mut()),
            -1
        );
        let missing = CString::new("no-such-ini").unwrap_or_else(|_| CString::new("x").unwrap());
        assert_eq!(ini_parse(missing.as_ptr(), None, std::ptr::null_mut()), -1);
        assert_eq!(
            ini_parse_file(std::ptr::null_mut(), Some(collect), std::ptr::null_mut()),
            -1
        );
        assert_eq!(
            ini_parse_stream(
                Some(mem_read),
                std::ptr::null_mut(),
                Some(collect),
                std::ptr::null_mut()
            ),
            -1
        );
        assert_eq!(
            ini_parse_stream(None, 1 as *mut c_void, Some(collect), std::ptr::null_mut()),
            -1
        );
        assert_eq!(
            ini_parse_string(std::ptr::null(), Some(collect), std::ptr::null_mut()),
            -1
        );
        assert_eq!(
            ini_parse_string_length(std::ptr::null(), 0, Some(collect), std::ptr::null_mut()),
            0
        );
        assert_eq!(
            ini_parse_string_length(std::ptr::null(), 4, Some(collect), std::ptr::null_mut()),
            -1
        );
        assert_eq!(
            ini_parse_string_length(std::ptr::null(), 0, None, std::ptr::null_mut()),
            -1
        );
    }
}

#[test]
fn missing_file_is_open_error() {
    let path = fixture("no_file.ini");
    let code = unsafe { ini_parse(path.as_ptr(), Some(collect), std::ptr::null_mut()) };
    assert_eq!(code, -1);
}

#[test]
fn value_may_be_mutated_during_handler() {
    let text = CString::new("[s]\na=bc\n").unwrap();
    let code = unsafe { ini_parse_string(text.as_ptr(), Some(mutate_value), std::ptr::null_mut()) };
    assert_eq!(code, 0);
}

#[test]
fn handler_zero_records_first_error_and_continues() {
    let text = CString::new("[s]\nuser=parse_error\nok=1\n").unwrap();
    let mut sink = Sink { events: Vec::new() };
    let code = unsafe {
        ini_parse_string(
            text.as_ptr(),
            Some(collect),
            &mut sink as *mut Sink as *mut c_void,
        )
    };
    assert_eq!(code, 2);
    assert_eq!(sink.events.len(), 2);
    assert_eq!(sink.events[1].1.as_deref(), Some(b"ok".as_slice()));
}

#[test]
fn five_entry_points_agree_on_normal_ini() {
    let path = fixture("normal.ini");
    let bytes =
        std::fs::read(std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../tests/normal.ini"))
            .unwrap_or_default();
    assert!(!bytes.is_empty(), "normal.ini did not load");
    let mut from_path = Sink { events: Vec::new() };
    let mut from_file = Sink { events: Vec::new() };
    let mut from_stream = Sink { events: Vec::new() };
    let mut from_string = Sink { events: Vec::new() };
    let mut from_length = Sink { events: Vec::new() };

    let c_path = unsafe {
        ini_parse(
            path.as_ptr(),
            Some(collect),
            &mut from_path as *mut Sink as *mut c_void,
        )
    };
    let c_file = unsafe {
        let f = libc::fopen(path.as_ptr(), c"r".as_ptr());
        assert!(!f.is_null());
        let code = ini_parse_file(f, Some(collect), &mut from_file as *mut Sink as *mut c_void);
        libc::fclose(f);
        code
    };
    let mut mem = Mem {
        data: {
            // Leak a copy so the reader can hold a 'static slice for the call.
            let leaked: &'static [u8] = Box::leak(bytes.clone().into_boxed_slice());
            leaked
        },
        pos: 0,
    };
    let c_stream = unsafe {
        ini_parse_stream(
            Some(mem_read),
            &mut mem as *mut Mem as *mut c_void,
            Some(collect),
            &mut from_stream as *mut Sink as *mut c_void,
        )
    };
    let c_string = unsafe {
        let owned = CString::new(bytes.clone()).unwrap();
        ini_parse_string(
            owned.as_ptr(),
            Some(collect),
            &mut from_string as *mut Sink as *mut c_void,
        )
    };
    let c_len = unsafe {
        ini_parse_string_length(
            bytes.as_ptr().cast(),
            bytes.len(),
            Some(collect),
            &mut from_length as *mut Sink as *mut c_void,
        )
    };
    assert_eq!(c_path, 0);
    assert_eq!(c_file, c_path);
    assert_eq!(c_stream, c_path);
    assert_eq!(c_string, c_path);
    assert_eq!(c_len, c_path);
    assert!(from_path.events.len() > 5);
    assert_eq!(from_path.events, from_file.events);
    assert_eq!(from_path.events, from_stream.events);
    assert_eq!(from_path.events, from_string.events);
    assert_eq!(from_path.events, from_length.events);
}
