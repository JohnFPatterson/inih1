//! Thin C ABI for the default `ini.h` configuration.
//!
//! Line-number handlers and the custom allocator are not part of this ABI.
//! They stay on `inih-core` because they change the C signature.

#![deny(unsafe_op_in_unsafe_fn)]
#![deny(clippy::undocumented_unsafe_blocks)]

use std::ffi::{c_char, c_int, c_void};

use inih_core::{parse_bytes, parse_reader, Config};

/// `ini_handler` with `INI_HANDLER_LINENO` left at 0.
pub type IniHandler = unsafe extern "C" fn(
    user: *mut c_void,
    section: *const c_char,
    name: *const c_char,
    value: *const c_char,
) -> c_int;

/// `ini_reader`.
pub type IniReader =
    unsafe extern "C" fn(buf: *mut c_char, num: c_int, stream: *mut c_void) -> *mut c_char;

/// `ini_parse`. A null filename or handler returns -1. C `assert`s on a null
/// handler inside `ini_parse_stream` and `fopen` on a null path is undefined;
/// both are rejected here.
///
/// # Safety
///
/// `filename` is null or a NUL-terminated path valid for this call. `handler`,
/// when `Some`, is an `ini_handler` and may be called with pointers that are
/// only valid during that callback. `user` is passed through unchanged.
#[no_mangle]
pub unsafe extern "C" fn ini_parse(
    filename: *const c_char,
    handler: Option<IniHandler>,
    user: *mut c_void,
) -> c_int {
    if filename.is_null() {
        return -1;
    }
    // SAFETY: filename is non-null and the caller guarantees it is a
    // NUL-terminated path for this call. `c"r"` is a static C string.
    let file = unsafe { libc::fopen(filename, c"r".as_ptr()) };
    if file.is_null() {
        return -1;
    }
    // SAFETY: file is the non-null FILE* just opened. handler and user are the
    // caller's values; ini_parse_file rejects a null handler.
    let code = unsafe { ini_parse_file(file, handler, user) };
    // SAFETY: file was opened by fopen and is non-null.
    unsafe { libc::fclose(file) };
    code
}

/// `ini_parse_file`. Does not close `file`.
///
/// # Safety
///
/// `file` is null or an open `FILE*` that stays open for this call. `handler`,
/// when `Some`, is an `ini_handler`. `user` is passed through unchanged.
#[no_mangle]
pub unsafe extern "C" fn ini_parse_file(
    file: *mut libc::FILE,
    handler: Option<IniHandler>,
    user: *mut c_void,
) -> c_int {
    if file.is_null() || handler.is_none() {
        return -1;
    }
    let Some(handler) = handler else {
        return -1;
    };
    parse_reader(
        &Config::default(),
        |dst| {
            if dst.len() > c_int::MAX as usize {
                return false;
            }
            // SAFETY: file is a non-null FILE* the caller keeps open. dst is a
            // writable buffer of dst.len() bytes. fgets writes at most that many
            // bytes, including the terminating NUL.
            let got = unsafe { libc::fgets(dst.as_mut_ptr().cast(), dst.len() as c_int, file) };
            !got.is_null()
        },
        |call| call_handler(handler, user, call.section, call.name, call.value),
        |_| true,
    )
}

/// `ini_parse_stream`.
///
/// # Safety
///
/// `reader`, when `Some`, is an `ini_reader`. `stream` is the pointer passed
/// to that reader; a null stream returns -1. `handler`, when `Some`, is an
/// `ini_handler`. `user` is passed through unchanged.
#[no_mangle]
pub unsafe extern "C" fn ini_parse_stream(
    reader: Option<IniReader>,
    stream: *mut c_void,
    handler: Option<IniHandler>,
    user: *mut c_void,
) -> c_int {
    let (Some(reader), Some(handler)) = (reader, handler) else {
        return -1;
    };
    if stream.is_null() {
        return -1;
    }
    parse_reader(
        &Config::default(),
        |dst| {
            if dst.len() > c_int::MAX as usize || dst.is_empty() {
                return false;
            }
            // SAFETY: reader is the caller-supplied ini_reader. dst is writable
            // for dst.len() bytes. stream is the non-null pointer the caller passed.
            let got = unsafe { reader(dst.as_mut_ptr().cast(), dst.len() as c_int, stream) };
            !got.is_null()
        },
        |call| call_handler(handler, user, call.section, call.name, call.value),
        |_| true,
    )
}

/// `ini_parse_string`. A null string returns -1.
///
/// # Safety
///
/// `string` is null or a NUL-terminated string valid for this call.
/// `handler`, when `Some`, is an `ini_handler`. `user` is passed through.
#[no_mangle]
pub unsafe extern "C" fn ini_parse_string(
    string: *const c_char,
    handler: Option<IniHandler>,
    user: *mut c_void,
) -> c_int {
    if string.is_null() {
        return -1;
    }
    // SAFETY: string is non-null and the caller guarantees it is NUL-terminated
    // for the duration of the call.
    let bytes = unsafe { std::ffi::CStr::from_ptr(string.cast()) };
    // SAFETY: string and handler are the caller's values. The length is the
    // byte length of that C string, which does not include the terminator.
    unsafe { ini_parse_string_length(string, bytes.to_bytes().len(), handler, user) }
}

/// `ini_parse_string_length`. A null string with length 0 yields an empty parse
/// (the C reader returns immediately). A null string with a positive length
/// returns -1 instead of dereferencing it.
///
/// # Safety
///
/// When `string` is non-null, it points at `length` readable bytes for this
/// call. `handler`, when `Some`, is an `ini_handler`. `user` is passed through.
#[no_mangle]
pub unsafe extern "C" fn ini_parse_string_length(
    string: *const c_char,
    length: usize,
    handler: Option<IniHandler>,
    user: *mut c_void,
) -> c_int {
    let Some(handler) = handler else {
        return -1;
    };
    if string.is_null() {
        return if length == 0 { 0 } else { -1 };
    }
    // SAFETY: string points at `length` readable bytes for the duration of the call.
    let bytes = unsafe { std::slice::from_raw_parts(string.cast::<u8>(), length) };
    parse_bytes(
        &Config::default(),
        bytes,
        |call| call_handler(handler, user, call.section, call.name, call.value),
        |_| true,
    )
}

fn call_handler(
    handler: IniHandler,
    user: *mut c_void,
    section: &[u8],
    name: Option<&[u8]>,
    value: Option<&[u8]>,
) -> i32 {
    let section_buf = nul_bytes(section);
    let name_buf = name.map(nul_bytes);
    let value_buf = value.map(nul_bytes);
    let name_ptr = name_buf
        .as_ref()
        .map(|b| b.as_ptr())
        .unwrap_or(std::ptr::null());
    let value_ptr = value_buf
        .as_ref()
        .map(|b| b.as_ptr())
        .unwrap_or(std::ptr::null());
    // SAFETY: handler is the non-null ini_handler supplied by the caller.
    // The three buffers are NUL-terminated and live for this call only, which
    // is the lifetime ini.h documents for the callback arguments.
    unsafe {
        handler(
            user,
            section_buf.as_ptr().cast(),
            name_ptr.cast(),
            value_ptr.cast(),
        )
    }
}

fn nul_bytes(bytes: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(bytes.len() + 1);
    out.extend_from_slice(bytes);
    out.push(0);
    out
}
