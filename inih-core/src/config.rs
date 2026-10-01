//! Compile-time `ini.h` flags as an explicit runtime config.

/// Flags that `ini.h` exposes as preprocessor macros.
///
/// Defaults match the header. The unit-test matrix in `tests/unittest.sh`
/// is the set of presets returned by [`configs`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Config {
    pub allow_multiline: bool,
    pub allow_bom: bool,
    pub start_comment_prefixes: String,
    pub allow_inline_comments: bool,
    pub inline_comment_prefixes: String,
    pub use_stack: bool,
    pub max_line: usize,
    pub max_section: usize,
    pub max_name: usize,
    pub allow_realloc: bool,
    pub initial_alloc: usize,
    pub stop_on_first_error: bool,
    pub call_handler_on_new_section: bool,
    pub allow_no_value: bool,
    pub handler_lineno: bool,
    pub custom_allocator: bool,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            allow_multiline: true,
            allow_bom: true,
            start_comment_prefixes: ";#".to_string(),
            allow_inline_comments: true,
            inline_comment_prefixes: ";".to_string(),
            use_stack: true,
            max_line: 200,
            max_section: 50,
            max_name: 50,
            allow_realloc: false,
            initial_alloc: 200,
            stop_on_first_error: false,
            call_handler_on_new_section: false,
            allow_no_value: false,
            handler_lineno: false,
            custom_allocator: false,
        }
    }
}

impl Config {
    fn with(mut self, edit: impl FnOnce(&mut Self)) -> Self {
        edit(&mut self);
        self
    }
}

/// Named presets in driver order. See `tools/DRIVER_FORMAT.md`.
pub fn configs() -> Vec<(&'static str, Config)> {
    vec![
        ("default", Config::default()),
        ("max_line_20", Config::default().with(|c| c.max_line = 20)),
        (
            "no_multiline",
            Config::default().with(|c| c.allow_multiline = false),
        ),
        (
            "no_inline_comments",
            Config::default().with(|c| c.allow_inline_comments = false),
        ),
        (
            "stop_on_first_error",
            Config::default().with(|c| c.stop_on_first_error = true),
        ),
        (
            "handler_lineno",
            Config::default().with(|c| c.handler_lineno = true),
        ),
        ("heap", Config::default().with(|c| c.use_stack = false)),
        (
            "heap_max_line",
            Config::default().with(|c| {
                c.use_stack = false;
                c.max_line = 20;
                c.initial_alloc = 20;
            }),
        ),
        (
            "heap_realloc",
            Config::default().with(|c| {
                c.use_stack = false;
                c.allow_realloc = true;
                c.initial_alloc = 5;
            }),
        ),
        (
            "heap_realloc_max_line",
            Config::default().with(|c| {
                c.use_stack = false;
                c.max_line = 20;
                c.allow_realloc = true;
                c.initial_alloc = 5;
            }),
        ),
        (
            "call_handler_on_new_section",
            Config::default().with(|c| c.call_handler_on_new_section = true),
        ),
        (
            "allow_no_value",
            Config::default().with(|c| c.allow_no_value = true),
        ),
        (
            "custom_alloc",
            Config::default().with(|c| {
                c.custom_allocator = true;
                c.use_stack = false;
                c.allow_realloc = true;
                c.initial_alloc = 12;
            }),
        ),
    ]
}
