# Parity drivers. Original ini.c / ini.h are compiled unchanged.
CC ?= gcc
ASAN_CC ?= gcc
CFLAGS ?= -Wall -Wextra -O2
TARGET_DIR ?= target

CONFIGS = \
	default \
	max_line_20 \
	no_multiline \
	no_inline_comments \
	stop_on_first_error \
	handler_lineno \
	heap \
	heap_max_line \
	heap_realloc \
	heap_realloc_max_line \
	call_handler_on_new_section \
	allow_no_value \
	custom_alloc

default_flags :=
max_line_20_flags := -DINI_MAX_LINE=20
no_multiline_flags := -DINI_ALLOW_MULTILINE=0
no_inline_comments_flags := -DINI_ALLOW_INLINE_COMMENTS=0
stop_on_first_error_flags := -DINI_STOP_ON_FIRST_ERROR=1
handler_lineno_flags := -DINI_HANDLER_LINENO=1
heap_flags := -DINI_USE_STACK=0
heap_max_line_flags := -DINI_USE_STACK=0 -DINI_MAX_LINE=20 -DINI_INITIAL_ALLOC=20
heap_realloc_flags := -DINI_USE_STACK=0 -DINI_ALLOW_REALLOC=1 -DINI_INITIAL_ALLOC=5
heap_realloc_max_line_flags := -DINI_USE_STACK=0 -DINI_MAX_LINE=20 -DINI_ALLOW_REALLOC=1 -DINI_INITIAL_ALLOC=5
call_handler_on_new_section_flags := -DINI_CALL_HANDLER_ON_NEW_SECTION=1
allow_no_value_flags := -DINI_ALLOW_NO_VALUE=1
custom_alloc_flags := -DINI_CUSTOM_ALLOCATOR=1 -DINI_USE_STACK=0 -DINI_ALLOW_REALLOC=1 -DINI_INITIAL_ALLOC=12

.PHONY: parity-build parity-check parity c-tests ffi-tests exports asan-oracle hook-trace clean-oracle

parity-build: $(addprefix build/oracle-,$(CONFIGS)) build/inih-oracle build/oracle-ffi $(TARGET_DIR)/release/inih-driver

build:
	mkdir -p build

build/oracle-%: tools/inih-oracle.c ini.c ini.h | build
	$(CC) $(CFLAGS) $($*_flags) -DINI_ORACLE_CONFIG='"$*"' -I. tools/inih-oracle.c ini.c -o $@.tmp
	mv $@.tmp $@

build/inih-oracle: tools/inih-dispatch.c | build
	$(CC) $(CFLAGS) tools/inih-dispatch.c -o $@.tmp
	mv $@.tmp $@

$(TARGET_DIR)/release/inih-driver: $(shell find inih-core inih-ffi -name '*.rs' -o -name 'Cargo.toml' 2>/dev/null) Cargo.toml
	cargo build --release --target-dir $(TARGET_DIR) -p inih-ffi -p inih-core --bin inih-driver

build/oracle-ffi: tools/inih-oracle.c ini.h $(TARGET_DIR)/release/libinih_ffi.a | build
	$(CC) $(CFLAGS) -DINI_ORACLE_CONFIG='"default"' -I. tools/inih-oracle.c $(TARGET_DIR)/release/libinih_ffi.a -lpthread -ldl -lm -o $@.tmp
	mv $@.tmp $@

$(TARGET_DIR)/release/libinih_ffi.a: $(shell find inih-core inih-ffi -name '*.rs' -o -name 'Cargo.toml' 2>/dev/null) Cargo.toml
	cargo build --release --target-dir $(TARGET_DIR) -p inih-ffi

parity:
	printf '%s' '{"status":"completed","loop_count":0,"workspace_roots":["$(CURDIR)"]}' | /home/ubuntu/.cursor/hooks/c-rust-parity/parity_gate.py --force

parity-check: parity-build
	@fail=0; \
	for f in tests/*.ini; do \
	  ./build/inih-oracle "$$f" > build/c.out; \
	  ./$(TARGET_DIR)/release/inih-driver "$$f" > build/r.out; \
	  if ! cmp -s build/c.out build/r.out; then \
	    echo "DIFF $$f"; diff -u build/c.out build/r.out | head -n 80; fail=1; \
	  else echo "ok $$f"; fi; \
	done; \
	./build/oracle-default tests/normal.ini > build/c-def.out; \
	./build/oracle-ffi tests/normal.ini > build/ffi.out; \
	if ! cmp -s build/c-def.out build/ffi.out; then \
	  echo "DIFF oracle-ffi normal.ini"; diff -u build/c-def.out build/ffi.out | head -n 40; fail=1; \
	else echo "ok oracle-ffi normal.ini"; fi; \
	exit $$fail

# Original unittest.sh overwrites baseline_*.txt. These targets diff instead.
c-tests: | build
	$(CC) $(CFLAGS) -I. ini.c tests/unittest.c -o build/unittest_multi.tmp && mv build/unittest_multi.tmp build/unittest_multi
	(cd tests && ../build/unittest_multi > ../build/out_multi.txt)
	diff -u tests/baseline_multi.txt build/out_multi.txt
	$(CC) $(CFLAGS) -I. -DINI_MAX_LINE=20 ini.c tests/unittest.c -o build/unittest_multi_max_line.tmp && mv build/unittest_multi_max_line.tmp build/unittest_multi_max_line
	(cd tests && ../build/unittest_multi_max_line > ../build/out_multi_max_line.txt)
	diff -u tests/baseline_multi_max_line.txt build/out_multi_max_line.txt
	$(CC) $(CFLAGS) -I. -DINI_ALLOW_MULTILINE=0 ini.c tests/unittest.c -o build/unittest_single.tmp && mv build/unittest_single.tmp build/unittest_single
	(cd tests && ../build/unittest_single > ../build/out_single.txt)
	diff -u tests/baseline_single.txt build/out_single.txt
	$(CC) $(CFLAGS) -I. -DINI_ALLOW_INLINE_COMMENTS=0 ini.c tests/unittest.c -o build/unittest_inline.tmp && mv build/unittest_inline.tmp build/unittest_inline
	(cd tests && ../build/unittest_inline > ../build/out_inline.txt)
	diff -u tests/baseline_disallow_inline_comments.txt build/out_inline.txt
	$(CC) $(CFLAGS) -I. -DINI_STOP_ON_FIRST_ERROR=1 ini.c tests/unittest.c -o build/unittest_stop.tmp && mv build/unittest_stop.tmp build/unittest_stop
	(cd tests && ../build/unittest_stop > ../build/out_stop.txt)
	diff -u tests/baseline_stop_on_first_error.txt build/out_stop.txt
	$(CC) $(CFLAGS) -I. -DINI_HANDLER_LINENO=1 ini.c tests/unittest.c -o build/unittest_lineno.tmp && mv build/unittest_lineno.tmp build/unittest_lineno
	(cd tests && ../build/unittest_lineno > ../build/out_lineno.txt)
	diff -u tests/baseline_handler_lineno.txt build/out_lineno.txt
	$(CC) $(CFLAGS) -I. -DINI_MAX_LINE=20 ini.c tests/unittest_string.c -o build/unittest_string.tmp && mv build/unittest_string.tmp build/unittest_string
	(cd tests && ../build/unittest_string > ../build/out_string.txt)
	diff -u tests/baseline_string.txt build/out_string.txt
	$(CC) $(CFLAGS) -I. -DINI_USE_STACK=0 ini.c tests/unittest.c -o build/unittest_heap.tmp && mv build/unittest_heap.tmp build/unittest_heap
	(cd tests && ../build/unittest_heap > ../build/out_heap.txt)
	diff -u tests/baseline_heap.txt build/out_heap.txt
	$(CC) $(CFLAGS) -I. -DINI_USE_STACK=0 -DINI_MAX_LINE=20 -DINI_INITIAL_ALLOC=20 ini.c tests/unittest.c -o build/unittest_heap_max.tmp && mv build/unittest_heap_max.tmp build/unittest_heap_max
	(cd tests && ../build/unittest_heap_max > ../build/out_heap_max.txt)
	diff -u tests/baseline_heap_max_line.txt build/out_heap_max.txt
	$(CC) $(CFLAGS) -I. -DINI_USE_STACK=0 -DINI_ALLOW_REALLOC=1 -DINI_INITIAL_ALLOC=5 ini.c tests/unittest.c -o build/unittest_realloc.tmp && mv build/unittest_realloc.tmp build/unittest_realloc
	(cd tests && ../build/unittest_realloc > ../build/out_realloc.txt)
	diff -u tests/baseline_heap_realloc.txt build/out_realloc.txt
	$(CC) $(CFLAGS) -I. -DINI_USE_STACK=0 -DINI_MAX_LINE=20 -DINI_ALLOW_REALLOC=1 -DINI_INITIAL_ALLOC=5 ini.c tests/unittest.c -o build/unittest_realloc_max.tmp && mv build/unittest_realloc_max.tmp build/unittest_realloc_max
	(cd tests && ../build/unittest_realloc_max > ../build/out_realloc_max.txt)
	diff -u tests/baseline_heap_realloc_max_line.txt build/out_realloc_max.txt
	$(CC) $(CFLAGS) -I. -DINI_USE_STACK=0 -DINI_MAX_LINE=20 -DINI_INITIAL_ALLOC=20 ini.c tests/unittest_string.c -o build/unittest_heap_string.tmp && mv build/unittest_heap_string.tmp build/unittest_heap_string
	(cd tests && ../build/unittest_heap_string > ../build/out_heap_string.txt)
	diff -u tests/baseline_heap_string.txt build/out_heap_string.txt
	$(CC) $(CFLAGS) -I. -DINI_CALL_HANDLER_ON_NEW_SECTION=1 ini.c tests/unittest.c -o build/unittest_newsec.tmp && mv build/unittest_newsec.tmp build/unittest_newsec
	(cd tests && ../build/unittest_newsec > ../build/out_newsec.txt)
	diff -u tests/baseline_call_handler_on_new_section.txt build/out_newsec.txt
	$(CC) $(CFLAGS) -I. -DINI_ALLOW_NO_VALUE=1 ini.c tests/unittest.c -o build/unittest_novalue.tmp && mv build/unittest_novalue.tmp build/unittest_novalue
	(cd tests && ../build/unittest_novalue > ../build/out_novalue.txt)
	diff -u tests/baseline_allow_no_value.txt build/out_novalue.txt
	$(CC) $(CFLAGS) -I. -DINI_CUSTOM_ALLOCATOR=1 -DINI_USE_STACK=0 -DINI_ALLOW_REALLOC=1 -DINI_INITIAL_ALLOC=12 ini.c tests/unittest_alloc.c -o build/unittest_alloc.tmp && mv build/unittest_alloc.tmp build/unittest_alloc
	(cd tests && ../build/unittest_alloc > ../build/out_alloc.txt)
	diff -u tests/baseline_alloc.txt build/out_alloc.txt

ffi-tests: $(TARGET_DIR)/release/libinih_ffi.a build/oracle-default build/oracle-ffi | build
	$(CC) $(CFLAGS) -I. tests/unittest.c $(TARGET_DIR)/release/libinih_ffi.a -lpthread -ldl -lm -o build/unittest_ffi.tmp && mv build/unittest_ffi.tmp build/unittest_ffi
	(cd tests && ../build/unittest_ffi > ../build/out_ffi.txt)
	diff -u tests/baseline_multi.txt build/out_ffi.txt
	$(CC) $(CFLAGS) -I. examples/ini_dump.c ini.c -o build/ini_dump_c.tmp && mv build/ini_dump_c.tmp build/ini_dump_c
	$(CC) $(CFLAGS) -I. examples/ini_dump.c $(TARGET_DIR)/release/libinih_ffi.a -lpthread -ldl -lm -o build/ini_dump_ffi.tmp && mv build/ini_dump_ffi.tmp build/ini_dump_ffi
	./build/ini_dump_c tests/normal.ini > build/dump_c.txt; echo $$? > build/dump_c.rc
	./build/ini_dump_ffi tests/normal.ini > build/dump_ffi.txt; echo $$? > build/dump_ffi.rc
	diff -u build/dump_c.txt build/dump_ffi.txt
	diff -u build/dump_c.rc build/dump_ffi.rc
	$(CC) $(CFLAGS) examples/ini_example.c ini.c -o build/ini_example_c.tmp && mv build/ini_example_c.tmp build/ini_example_c
	$(CC) $(CFLAGS) examples/ini_example.c $(TARGET_DIR)/release/libinih_ffi.a -lpthread -ldl -lm -o build/ini_example_ffi.tmp && mv build/ini_example_ffi.tmp build/ini_example_ffi
	(cd examples && ../build/ini_example_c > ../build/example_c.txt)
	(cd examples && ../build/ini_example_ffi > ../build/example_ffi.txt)
	diff -u build/example_c.txt build/example_ffi.txt
	$(CC) $(CFLAGS) examples/ini_xmacros.c ini.c -o build/ini_xmacros_c.tmp && mv build/ini_xmacros_c.tmp build/ini_xmacros_c
	$(CC) $(CFLAGS) examples/ini_xmacros.c $(TARGET_DIR)/release/libinih_ffi.a -lpthread -ldl -lm -o build/ini_xmacros_ffi.tmp && mv build/ini_xmacros_ffi.tmp build/ini_xmacros_ffi
	(cd examples && ../build/ini_xmacros_c > ../build/xmacros_c.txt)
	(cd examples && ../build/ini_xmacros_ffi > ../build/xmacros_ffi.txt)
	diff -u build/xmacros_c.txt build/xmacros_ffi.txt
	@fail=0; \
	for f in tests/*.ini; do \
	  ./build/oracle-default "$$f" > build/c-def.out; \
	  ./build/oracle-ffi "$$f" > build/ffi.out; \
	  if ! cmp -s build/c-def.out build/ffi.out; then echo "DIFF oracle-ffi $$f"; fail=1; else echo "ok oracle-ffi $$f"; fi; \
	done; \
	exit $$fail

exports: $(TARGET_DIR)/release/libinih_ffi.a | build
	grep -E '^INI_API int ini_' ini.h | sed -E 's/^INI_API int ([A-Za-z_][A-Za-z0-9_]*).*/\1/' | sort -u > build/header-fns.txt
	nm -g --defined-only $(TARGET_DIR)/release/libinih_ffi.a | awk 'NF>=3 && $$2=="T" {print $$3}' | sed 's/^_//' | sort -u > build/ffi-exports.txt
	@echo "header functions: $$(wc -l < build/header-fns.txt)"
	@test "$$(wc -l < build/header-fns.txt)" -eq 5
	@comm -23 build/header-fns.txt build/ffi-exports.txt > build/missing-exports.txt
	@if [ -s build/missing-exports.txt ]; then echo "missing exports:"; cat build/missing-exports.txt; exit 1; fi
	@echo "exports ok"

build/hook-c-default: tools/hook-trace.c ini.c ini.h | build
	$(CC) $(CFLAGS) -DINI_HOOK_CONFIG='"default"' -I. tools/hook-trace.c ini.c -o $@.tmp
	mv $@.tmp $@

build/hook-c-custom: tools/hook-trace.c ini.c ini.h | build
	$(CC) $(CFLAGS) -DINI_CUSTOM_ALLOCATOR=1 -DINI_USE_STACK=0 -DINI_ALLOW_REALLOC=1 -DINI_INITIAL_ALLOC=12 -DINI_HOOK_CONFIG='"custom_alloc"' -I. tools/hook-trace.c ini.c -o $@.tmp
	mv $@.tmp $@

build/hook-ffi: tools/hook-trace.c $(TARGET_DIR)/release/libinih_ffi.a | build
	$(CC) $(CFLAGS) -DINI_HOOK_CONFIG='"default"' -I. tools/hook-trace.c $(TARGET_DIR)/release/libinih_ffi.a -lpthread -ldl -lm -o $@.tmp
	mv $@.tmp $@

$(TARGET_DIR)/release/inih-hook-trace: $(shell find inih-core inih-ffi -name '*.rs' -o -name 'Cargo.toml' 2>/dev/null) Cargo.toml
	cargo build --release --target-dir $(TARGET_DIR) -p inih-core --bin inih-hook-trace

hook-trace: build/hook-c-default build/hook-c-custom build/hook-ffi $(TARGET_DIR)/release/inih-hook-trace
	@fail=0; \
	for n in 0 1; do \
	  ./build/hook-c-default --fail $$n > build/hook-c-default-$$n.txt; \
	  ./build/hook-ffi --fail $$n > build/hook-ffi-$$n.txt; \
	  ./$(TARGET_DIR)/release/inih-hook-trace --config default --fail $$n > build/hook-rs-default-$$n.txt; \
	  if ! cmp -s build/hook-c-default-$$n.txt build/hook-ffi-$$n.txt; then echo "DIFF hook ffi fail $$n"; diff -u build/hook-c-default-$$n.txt build/hook-ffi-$$n.txt | head -n 40; fail=1; \
	  else echo "ok hook ffi fail $$n"; fi; \
	  if ! cmp -s build/hook-c-default-$$n.txt build/hook-rs-default-$$n.txt; then echo "DIFF hook rust default fail $$n"; diff -u build/hook-c-default-$$n.txt build/hook-rs-default-$$n.txt | head -n 40; fail=1; \
	  else echo "ok hook rust default fail $$n"; fi; \
	done; \
	for n in 0 1 2 3 4 5 6 7 8; do \
	  ./build/hook-c-custom --fail $$n > build/hook-c-custom-$$n.txt; \
	  ./$(TARGET_DIR)/release/inih-hook-trace --config custom_alloc --fail $$n > build/hook-rs-custom-$$n.txt; \
	  if ! cmp -s build/hook-c-custom-$$n.txt build/hook-rs-custom-$$n.txt; then echo "DIFF hook custom fail $$n"; diff -u build/hook-c-custom-$$n.txt build/hook-rs-custom-$$n.txt | head -n 50; fail=1; \
	  else echo "ok hook custom fail $$n"; fi; \
	done; \
	exit $$fail

build/asan:
	mkdir -p build/asan

build/asan/oracle-%: tools/inih-oracle.c ini.c ini.h | build/asan
	$(ASAN_CC) -g -fsanitize=address,undefined -fno-omit-frame-pointer $($*_flags) -DINI_ORACLE_CONFIG='"$*"' -I. tools/inih-oracle.c ini.c -o $@.tmp
	mv $@.tmp $@

build/asan/unittest_multi: ini.c tests/unittest.c ini.h | build/asan
	$(ASAN_CC) -g -fsanitize=address,undefined -fno-omit-frame-pointer -I. ini.c tests/unittest.c -o $@.tmp
	mv $@.tmp $@

build/asan/hook-custom: tools/hook-trace.c ini.c ini.h | build/asan
	$(ASAN_CC) -g -fsanitize=address,undefined -fno-omit-frame-pointer -DINI_CUSTOM_ALLOCATOR=1 -DINI_USE_STACK=0 -DINI_ALLOW_REALLOC=1 -DINI_INITIAL_ALLOC=12 -DINI_HOOK_CONFIG='"custom_alloc"' -I. tools/hook-trace.c ini.c -o $@.tmp
	mv $@.tmp $@

asan-oracle: $(addprefix build/asan/oracle-,$(CONFIGS)) build/asan/unittest_multi build/asan/hook-custom
	@fail=0; \
	for name in $(CONFIGS); do \
	  for f in tests/*.ini; do \
	    if ! ASAN_OPTIONS=detect_leaks=0 ./build/asan/oracle-$$name "$$f" > /dev/null; then echo "ASAN: $$name $$f"; fail=1; fi; \
	  done; \
	  echo "asan oracle $$name ok"; \
	done; \
	if ! (cd tests && ASAN_OPTIONS=detect_leaks=0 ../build/asan/unittest_multi > /dev/null); then echo "ASAN: unittest_multi"; fail=1; else echo "asan unittest_multi ok"; fi; \
	if ! ASAN_OPTIONS=detect_leaks=0 ./build/asan/hook-custom --fail 0 > /dev/null; then echo "ASAN: hook-custom"; fail=1; else echo "asan hook-custom ok"; fi; \
	if ! ASAN_OPTIONS=detect_leaks=0 ./build/asan/hook-custom --fail 2 > /dev/null; then echo "ASAN: hook-custom fail 2"; fail=1; else echo "asan hook-custom fail 2 ok"; fi; \
	exit $$fail

clean-oracle:
	rm -rf build/oracle-* build/inih-oracle build/oracle-ffi build/unittest_* build/ini_dump_* build/ini_example_* build/ini_xmacros_*
