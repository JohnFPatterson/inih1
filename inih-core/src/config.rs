//! Runtime form of the `ini.h` compile-time switches.

/// Parser limits and flags. Defaults match `ini.h`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Config {
    pub allow_multiline: bool,
    pub allow_bom: bool,
    pub start_comment_prefixes: &'static [u8],
    pub allow_inline_comments: bool,
    pub inline_comment_prefixes: &'static [u8],
    /// When true, the line buffer is the stack size `max_line` and no
    /// allocator events are produced. When false, the buffer starts at
    /// `initial_alloc` and may grow if `allow_realloc` is set.
    pub use_stack: bool,
    pub max_line: usize,
    pub max_section: usize,
    pub max_name: usize,
    pub allow_realloc: bool,
    pub initial_alloc: usize,
    pub stop_on_first_error: bool,
    pub call_handler_on_new_section: bool,
    pub allow_no_value: bool,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            allow_multiline: true,
            allow_bom: true,
            start_comment_prefixes: b";#",
            allow_inline_comments: true,
            inline_comment_prefixes: b";",
            use_stack: true,
            max_line: 200,
            max_section: 50,
            max_name: 50,
            allow_realloc: false,
            initial_alloc: 200,
            stop_on_first_error: false,
            call_handler_on_new_section: false,
            allow_no_value: false,
        }
    }
}

impl Config {
    pub(crate) fn limits_ok(&self) -> bool {
        self.max_line >= 2
            && self.max_section >= 2
            && self.max_name >= 2
            && (self.use_stack || self.initial_alloc >= 2)
    }
}

/// Named configurations from `tests/unittest.sh`. `handler_lineno` parses like
/// `multi`; only the callback signature differs, which the driver prints.
pub fn preset(name: &str) -> Option<Config> {
    let mut cfg = Config::default();
    match name {
        "multi" | "handler_lineno" => {}
        "multi_max_line" | "string" => cfg.max_line = 20,
        "single" => cfg.allow_multiline = false,
        "disallow_inline_comments" => cfg.allow_inline_comments = false,
        "stop_on_first_error" => cfg.stop_on_first_error = true,
        "heap" => cfg.use_stack = false,
        "heap_max_line" | "heap_string" => {
            cfg.use_stack = false;
            cfg.max_line = 20;
            cfg.initial_alloc = 20;
        }
        "heap_realloc" => {
            cfg.use_stack = false;
            cfg.allow_realloc = true;
            cfg.initial_alloc = 5;
        }
        "heap_realloc_max_line" => {
            cfg.use_stack = false;
            cfg.max_line = 20;
            cfg.allow_realloc = true;
            cfg.initial_alloc = 5;
        }
        "call_handler_on_new_section" => cfg.call_handler_on_new_section = true,
        "allow_no_value" => cfg.allow_no_value = true,
        "alloc" => {
            cfg.use_stack = false;
            cfg.allow_realloc = true;
            cfg.initial_alloc = 12;
        }
        _ => return None,
    }
    Some(cfg)
}
