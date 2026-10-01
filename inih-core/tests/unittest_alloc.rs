//! Case-for-case port of `tests/unittest_alloc.c`.

#[path = "common/mod.rs"]
mod support;

use support::run_alloc_suite;

#[test]
fn unittest_alloc() {
    run_alloc_suite("baseline_alloc.txt");
}
