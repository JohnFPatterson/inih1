//! Safe port of the inih parser. Behavior follows `ini.c` for every
//! configuration `tests/unittest.sh` builds. Malformed input returns a code;
//! it does not panic.

#![forbid(unsafe_code)]

mod config;
mod parse;

pub use config::{preset, Config};
pub use parse::{
    classify, parse_bytes, parse_cstr, parse_path, parse_reader, read_fgets, AllocOp, Call, Error,
};
