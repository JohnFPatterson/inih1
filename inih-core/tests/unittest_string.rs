//! Case-for-case port of `tests/unittest_string.c`.

#[path = "common/mod.rs"]
mod support;

use support::{preset, run_string_suite};

#[test]
fn unittest_string() {
    run_string_suite(&preset("max_line_20"), "baseline_string.txt");
}

#[test]
fn unittest_heap_string() {
    run_string_suite(&preset("heap_max_line"), "baseline_heap_string.txt");
}
