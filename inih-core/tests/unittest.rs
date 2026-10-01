//! Case-for-case port of `tests/unittest.c` and the `tests/unittest.sh` matrix
//! that compiles that file.

#[path = "common/mod.rs"]
mod support;

use support::{preset, run_file_suite};

#[test]
fn unittest_multi() {
    run_file_suite(&preset("default"), "baseline_multi.txt");
}

#[test]
fn unittest_multi_max_line() {
    run_file_suite(&preset("max_line_20"), "baseline_multi_max_line.txt");
}

#[test]
fn unittest_single() {
    run_file_suite(&preset("no_multiline"), "baseline_single.txt");
}

#[test]
fn unittest_disallow_inline_comments() {
    run_file_suite(
        &preset("no_inline_comments"),
        "baseline_disallow_inline_comments.txt",
    );
}

#[test]
fn unittest_stop_on_first_error() {
    run_file_suite(
        &preset("stop_on_first_error"),
        "baseline_stop_on_first_error.txt",
    );
}

#[test]
fn unittest_handler_lineno() {
    run_file_suite(&preset("handler_lineno"), "baseline_handler_lineno.txt");
}

#[test]
fn unittest_heap() {
    run_file_suite(&preset("heap"), "baseline_heap.txt");
}

#[test]
fn unittest_heap_max_line() {
    run_file_suite(&preset("heap_max_line"), "baseline_heap_max_line.txt");
}

#[test]
fn unittest_heap_realloc() {
    run_file_suite(&preset("heap_realloc"), "baseline_heap_realloc.txt");
}

#[test]
fn unittest_heap_realloc_max_line() {
    run_file_suite(
        &preset("heap_realloc_max_line"),
        "baseline_heap_realloc_max_line.txt",
    );
}

#[test]
fn unittest_call_handler_on_new_section() {
    run_file_suite(
        &preset("call_handler_on_new_section"),
        "baseline_call_handler_on_new_section.txt",
    );
}

#[test]
fn unittest_allow_no_value() {
    run_file_suite(&preset("allow_no_value"), "baseline_allow_no_value.txt");
}
