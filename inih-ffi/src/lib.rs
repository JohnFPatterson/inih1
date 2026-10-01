//! Thin C ABI for the default `ini.h` configuration.
//!
//! `INI_HANDLER_LINENO` is 0 and every other header default is unchanged.
//! Non-default `tests/unittest.sh` flags are [`inih_core::Config`] values on
//! the core crate; they are not extra exported symbols.

#![deny(unsafe_op_in_unsafe_fn)]
#![deny(clippy::undocumented_unsafe_blocks)]

use std::os::raw::{c_char, c_int, c_void};
use std::process::abort;
use std::ptr;

use inih_core::{parse_bytes, parse_cstr, parse_reader, return_code, Config, IniRead, NopAlloc};

/// Opaque stand-in for C `FILE`. Only the pointer is passed across the ABI.
#[repr(C)]
pub struct CFile {
    _private: [u8; 0],
}

extern "C" {
    fn fopen(filename: *const c_char, mode: *const c_char) -> *mut CFile;
    fn fclose(stream: *mut CFile) -> c_int;
    fn fgets(s: *mut c_char, n: c_int, stream: *mut CFile) -> *mut c_char;
    fn strlen(s: *const c_char) -> usize;
}

pub type IniHandler = Option<
    unsafe extern "C" fn(
        user: *mut c_void,
        section: *const c_char,
        name: *const c_char,
        value: *const c_char,
    ) -> c_int,
>;

pub type IniReader =
    Option<unsafe extern "C" fn(str: *mut c_char, num: c_int, stream: *mut c_void) -> *mut c_char>;

struct FileReader(*mut CFile);

impl IniRead for FileReader {
    fn read(&mut self, dest: &mut [u8]) -> bool {
        if dest.len() < 2 || dest.len() > c_int::MAX as usize {
            return false;
        }
        // SAFETY: `dest` is a writable buffer of `dest.len()` bytes. `self.0` is the
        // `FILE*` passed by the caller of `ini_parse_file` / `ini_parse` and kept open
        // for this call. `fgets` writes at most `dest.len()` bytes and a NUL.
        let rc = unsafe { fgets(dest.as_mut_ptr().cast(), dest.len() as c_int, self.0) };
        !rc.is_null()
    }
}

struct FnReader {
    reader: unsafe extern "C" fn(*mut c_char, c_int, *mut c_void) -> *mut c_char,
    stream: *mut c_void,
}

impl IniRead for FnReader {
    fn read(&mut self, dest: &mut [u8]) -> bool {
        if dest.len() < 2 || dest.len() > c_int::MAX as usize {
            return false;
        }
        // SAFETY: `dest` is writable and `dest.len()` fits in `c_int`. `reader` and
        // `stream` are the pair supplied to `ini_parse_stream` and are only used
        // for this call, matching `ini_reader`.
        let rc =
            unsafe { (self.reader)(dest.as_mut_ptr().cast(), dest.len() as c_int, self.stream) };
        !rc.is_null()
    }
}

fn nul_buf(bytes: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(bytes.len() + 1);
    out.extend_from_slice(bytes);
    out.push(0);
    out
}

fn call_handler(
    handler: unsafe extern "C" fn(
        *mut c_void,
        *const c_char,
        *const c_char,
        *const c_char,
    ) -> c_int,
    user: *mut c_void,
    section: &[u8],
    name: Option<&[u8]>,
    value: Option<&[u8]>,
) -> bool {
    let section = nul_buf(section);
    let name = name.map(nul_buf);
    let value = value.map(nul_buf);
    let name_ptr = name
        .as_ref()
        .map(|b| b.as_ptr().cast())
        .unwrap_or(ptr::null());
    let value_ptr = value
        .as_ref()
        .map(|b| b.as_ptr().cast())
        .unwrap_or(ptr::null());
    // SAFETY: the three buffers are NUL-terminated and live for this call. NULL name
    // or value matches `INI_CALL_HANDLER_ON_NEW_SECTION` and `INI_ALLOW_NO_VALUE`.
    // `user` is the pointer the caller passed and is not dereferenced here.
    let rc = unsafe { handler(user, section.as_ptr().cast(), name_ptr, value_ptr) };
    rc != 0
}

fn finish(result: Result<(), inih_core::Error>) -> c_int {
    return_code(&result)
}

/// `ini_parse` from `ini.h`.
///
/// # Safety
/// `filename` must be a NUL-terminated path, or NULL (which aborts, as `fopen` does).
/// `handler` must be a valid callback or NULL (which aborts, as `ini.c`'s assert does).
#[no_mangle]
pub unsafe extern "C" fn ini_parse(
    filename: *const c_char,
    handler: IniHandler,
    user: *mut c_void,
) -> c_int {
    if filename.is_null() {
        abort();
    }
    // SAFETY: `filename` is NUL-terminated. The mode string is a static NUL-terminated `r`.
    let file = unsafe { fopen(filename, c"r".as_ptr()) };
    if file.is_null() {
        return -1;
    }
    // SAFETY: `file` was just opened and is closed below. `handler` / `user` are forwarded.
    let rc = unsafe { ini_parse_file(file, handler, user) };
    // SAFETY: `file` is the pointer `fopen` just returned.
    unsafe {
        fclose(file);
    }
    rc
}

/// `ini_parse_file` from `ini.h`. Does not close `file`.
///
/// # Safety
/// `file` must be an open `FILE*`. `handler` must be valid or NULL (abort).
#[no_mangle]
pub unsafe extern "C" fn ini_parse_file(
    file: *mut CFile,
    handler: IniHandler,
    user: *mut c_void,
) -> c_int {
    let Some(handler) = handler else {
        abort();
    };
    if file.is_null() {
        abort();
    }
    let mut reader = FileReader(file);
    let mut alloc = NopAlloc;
    let cfg = Config::default();
    let mut cb = |section: &[u8], name: Option<&[u8]>, value: Option<&[u8]>, _lineno: i32| {
        call_handler(handler, user, section, name, value)
    };
    finish(parse_reader(&mut reader, &cfg, &mut cb, &mut alloc))
}

/// `ini_parse_stream` from `ini.h`.
///
/// # Safety
/// `reader` and `stream` must match `ini_reader`. `handler` must be valid or NULL (abort).
#[no_mangle]
pub unsafe extern "C" fn ini_parse_stream(
    reader: IniReader,
    stream: *mut c_void,
    handler: IniHandler,
    user: *mut c_void,
) -> c_int {
    let Some(reader) = reader else {
        abort();
    };
    let Some(handler) = handler else {
        abort();
    };
    let mut fn_reader = FnReader { reader, stream };
    let mut alloc = NopAlloc;
    let cfg = Config::default();
    let mut cb = |section: &[u8], name: Option<&[u8]>, value: Option<&[u8]>, _lineno: i32| {
        call_handler(handler, user, section, name, value)
    };
    finish(parse_reader(&mut fn_reader, &cfg, &mut cb, &mut alloc))
}

/// `ini_parse_string` from `ini.h`.
///
/// # Safety
/// `string` must be NUL-terminated, or NULL (which aborts, as `strlen` does).
#[no_mangle]
pub unsafe extern "C" fn ini_parse_string(
    string: *const c_char,
    handler: IniHandler,
    user: *mut c_void,
) -> c_int {
    if string.is_null() {
        abort();
    }
    let Some(handler) = handler else {
        abort();
    };
    // SAFETY: `string` is NUL-terminated. `strlen` is that length, excluding the
    // terminator, and the slice covers exactly those bytes.
    let bytes = unsafe {
        let len = strlen(string);
        std::slice::from_raw_parts(string.cast::<u8>(), len)
    };
    let mut alloc = NopAlloc;
    let cfg = Config::default();
    let mut cb = |section: &[u8], name: Option<&[u8]>, value: Option<&[u8]>, _lineno: i32| {
        call_handler(handler, user, section, name, value)
    };
    finish(parse_cstr(bytes, &cfg, &mut cb, &mut alloc))
}

/// `ini_parse_string_length` from `ini.h`.
///
/// # Safety
/// When `length > 0`, `string` must point at `length` readable bytes.
/// A NULL `string` with `length == 0` parses as empty, matching the C reader.
#[no_mangle]
pub unsafe extern "C" fn ini_parse_string_length(
    string: *const c_char,
    length: usize,
    handler: IniHandler,
    user: *mut c_void,
) -> c_int {
    let Some(handler) = handler else {
        abort();
    };
    if string.is_null() && length > 0 {
        abort();
    }
    let bytes = if length == 0 {
        &[][..]
    } else {
        // SAFETY: caller guarantees `length` readable bytes at `string` for this call.
        unsafe { std::slice::from_raw_parts(string.cast::<u8>(), length) }
    };
    let mut alloc = NopAlloc;
    let cfg = Config::default();
    let mut cb = |section: &[u8], name: Option<&[u8]>, value: Option<&[u8]>, _lineno: i32| {
        call_handler(handler, user, section, name, value)
    };
    finish(parse_bytes(bytes, &cfg, &mut cb, &mut alloc))
}
