# Build the C oracle and the Rust driver. `make parity` runs the parity gate.
CC ?= gcc
CFLAGS ?= -Wall -Wextra -std=c11
ROOT := $(abspath .)

CFG_OBJS := \
	build/cfg_multi_max_line.o \
	build/cfg_single.o \
	build/cfg_disallow_inline_comments.o \
	build/cfg_stop_on_first_error.o \
	build/cfg_handler_lineno.o \
	build/cfg_string.o \
	build/cfg_heap.o \
	build/cfg_heap_max_line.o \
	build/cfg_heap_realloc.o \
	build/cfg_heap_realloc_max_line.o \
	build/cfg_heap_string.o \
	build/cfg_call_handler_on_new_section.o \
	build/cfg_allow_no_value.o \
	build/cfg_alloc.o

.PHONY: drivers parity oracle oracle-ffi asan hook-trace c-tests ffi-tests check fmt

drivers: oracle
	cargo build --release --target-dir target

oracle: build/oracle

build/cfgs.stamp: ini.c ini.h tools/compile_configs.sh
	bash tools/compile_configs.sh
	touch build/cfgs.stamp

build/ini.o $(CFG_OBJS): build/cfgs.stamp

build/oracle: tools/inih-oracle.c build/ini.o $(CFG_OBJS)
	mkdir -p build
	$(CC) $(CFLAGS) -I. -o build/oracle.tmp tools/inih-oracle.c build/ini.o $(CFG_OBJS)
	mv build/oracle.tmp build/oracle

oracle-ffi: drivers build/cfgs.stamp
	mkdir -p build
	$(CC) $(CFLAGS) -I. -o build/oracle-ffi.tmp tools/inih-oracle.c $(CFG_OBJS) \
		target/release/libinih_ffi.a -ldl -lpthread -lm
	mv build/oracle-ffi.tmp build/oracle-ffi

parity: drivers oracle-ffi
	printf '%s' '{"status":"completed","loop_count":0,"workspace_roots":["$(ROOT)"]}' \
		| ./.cursor/hooks/c-rust-parity/parity_gate.py --force
	$(MAKE) hook-trace

c-tests:
	bash -c 'cd tests && bash unittest.sh && git diff --exit-code -- baseline_*.txt'

ffi-tests: drivers
	mkdir -p build
	$(CC) $(CFLAGS) -I. -o build/unittest_ffi.tmp tests/unittest.c target/release/libinih_ffi.a -ldl -lpthread -lm
	mv build/unittest_ffi.tmp build/unittest_ffi
	cd tests && ../build/unittest_ffi > ../build/unittest_ffi.txt
	diff -u tests/baseline_multi.txt build/unittest_ffi.txt

ASAN_CFLAGS := -g -fsanitize=address,undefined -fno-omit-frame-pointer -Wall -Wextra -std=c11
ASAN_OBJS := $(patsubst build/%,build/asan/%,$(CFG_OBJS)) build/asan/ini.o

asan:
	mkdir -p build/asan
	OUTDIR=build/asan CFLAGS="$(ASAN_CFLAGS)" bash tools/compile_configs.sh
	$(CC) $(ASAN_CFLAGS) -I. -o build/asan/oracle.tmp tools/inih-oracle.c $(ASAN_OBJS)
	mv build/asan/oracle.tmp build/asan/oracle
	$(CC) $(ASAN_CFLAGS) -I. -o build/asan/hook-trace.tmp tools/hook-trace.c ini.c
	mv build/asan/hook-trace.tmp build/asan/hook-trace
	@set -e; \
	for f in tests/*.ini; do \
	  ASAN_OPTIONS=detect_leaks=1:halt_on_error=1 build/asan/oracle "$$f" > /dev/null; \
	  ASAN_OPTIONS=detect_leaks=1:halt_on_error=1 build/asan/hook-trace "$$f" > /dev/null; \
	done; \
	set -e; \
	asan_one() { \
	  name="$$1"; shift; \
	  $(CC) $(ASAN_CFLAGS) -I. "$$@" -o build/asan/$$name.tmp; \
	  mv build/asan/$$name.tmp build/asan/$$name; \
	  (cd tests && ASAN_OPTIONS=detect_leaks=1:halt_on_error=1 ../build/asan/$$name > ../build/asan/$$name.txt); \
	  diff -q tests/baseline_$$name.txt build/asan/$$name.txt; \
	}; \
	asan_one multi ini.c tests/unittest.c; \
	asan_one multi_max_line -DINI_MAX_LINE=20 ini.c tests/unittest.c; \
	asan_one single -DINI_ALLOW_MULTILINE=0 ini.c tests/unittest.c; \
	asan_one disallow_inline_comments -DINI_ALLOW_INLINE_COMMENTS=0 ini.c tests/unittest.c; \
	asan_one stop_on_first_error -DINI_STOP_ON_FIRST_ERROR=1 ini.c tests/unittest.c; \
	asan_one handler_lineno -DINI_HANDLER_LINENO=1 ini.c tests/unittest.c; \
	asan_one string -DINI_MAX_LINE=20 ini.c tests/unittest_string.c; \
	asan_one heap -DINI_USE_STACK=0 ini.c tests/unittest.c; \
	asan_one heap_max_line -DINI_USE_STACK=0 -DINI_MAX_LINE=20 -DINI_INITIAL_ALLOC=20 ini.c tests/unittest.c; \
	asan_one heap_realloc -DINI_USE_STACK=0 -DINI_ALLOW_REALLOC=1 -DINI_INITIAL_ALLOC=5 ini.c tests/unittest.c; \
	asan_one heap_realloc_max_line -DINI_USE_STACK=0 -DINI_MAX_LINE=20 -DINI_ALLOW_REALLOC=1 -DINI_INITIAL_ALLOC=5 ini.c tests/unittest.c; \
	asan_one heap_string -DINI_USE_STACK=0 -DINI_MAX_LINE=20 -DINI_INITIAL_ALLOC=20 ini.c tests/unittest_string.c; \
	asan_one call_handler_on_new_section -DINI_CALL_HANDLER_ON_NEW_SECTION=1 ini.c tests/unittest.c; \
	asan_one allow_no_value -DINI_ALLOW_NO_VALUE=1 ini.c tests/unittest.c; \
	asan_one alloc -DINI_CUSTOM_ALLOCATOR=1 -DINI_USE_STACK=0 -DINI_ALLOW_REALLOC=1 -DINI_INITIAL_ALLOC=12 ini.c tests/unittest_alloc.c; \
	echo "asan: clean"

exports: drivers
	mkdir -p build
	python3 tools/header_fns.py | sort > build/header-fns.txt
	nm -g --defined-only target/release/libinih_ffi.a | awk '$$2=="T"{print $$3}' | sed 's/^_//' | sort -u > build/ffi-exports.txt
	@test "$$(wc -l < build/header-fns.txt)" -eq 5
	@comm -23 build/header-fns.txt build/ffi-exports.txt > build/ffi-missing.txt
	@test ! -s build/ffi-missing.txt
	@echo "exports: 5/5"

hook-trace: oracle oracle-ffi drivers
	mkdir -p build
	$(CC) $(CFLAGS) -I. -o build/hook-trace-c.tmp tools/hook-trace.c ini.c
	mv build/hook-trace-c.tmp build/hook-trace-c
	$(CC) $(CFLAGS) -I. -o build/hook-trace-ffi.tmp tools/hook-trace.c target/release/libinih_ffi.a -ldl -lpthread -lm
	mv build/hook-trace-ffi.tmp build/hook-trace-ffi
	@set -e; \
	for f in tests/*.ini; do \
	  ./build/hook-trace-c "$$f" > build/hook-c.txt; \
	  ./build/hook-trace-ffi "$$f" > build/hook-ffi.txt; \
	  diff -u build/hook-c.txt build/hook-ffi.txt; \
	done; \
	for n in 0 1 2 3 4; do \
	  ./build/oracle --sections alloc --alloc-fail $$n tests/normal.ini > build/alloc-c.txt; \
	  ./target/release/rust-driver --sections alloc --alloc-fail $$n tests/normal.ini > build/alloc-rs.txt; \
	  diff -u build/alloc-c.txt build/alloc-rs.txt; \
	  ./build/oracle --sections allocfile --alloc-fail $$n tests/long_line.ini > build/allocf-c.txt; \
	  ./target/release/rust-driver --sections allocfile --alloc-fail $$n tests/long_line.ini > build/allocf-rs.txt; \
	  diff -u build/allocf-c.txt build/allocf-rs.txt; \
	done; \
	echo "hook-trace: clean"

check: drivers
	cargo test --workspace --all-targets --target-dir target
	cargo clippy --workspace --all-targets --target-dir target -- -D warnings
	cargo fmt --all --check

fmt:
	cargo fmt --all
