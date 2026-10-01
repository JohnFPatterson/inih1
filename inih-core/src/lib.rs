//! Safe port of the [inih](https://github.com/benhoyt/inih) parser.
//!
//! Behavior follows `ini.c` for every flag in [`Config`], including quirks.
//! Malformed input returns [`Error`]; library paths do not panic on it.

#![forbid(unsafe_code)]

mod config;
mod parse;

pub use config::{configs, Config};
pub use parse::{
    parse_bytes, parse_cstr, parse_path, parse_reader, return_code, AllocEvent, AllocOp, AllocSink,
    Error, Handler, IniRead, NopAlloc, RecordingAlloc, SliceReader,
};
