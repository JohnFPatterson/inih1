#!/bin/bash
# Compile one ini.c translation unit per tests/unittest.sh configuration.
# Symbol names are renamed so they can live in one oracle binary.
set -euo pipefail
cd "$(dirname "$0")/.."
OUTDIR="${OUTDIR:-build}"
mkdir -p "$OUTDIR"
CC="${CC:-gcc}"
CFLAGS="${CFLAGS:--Wall -Wextra -std=c11}"

compile() {
  local name="$1"
  shift
  local tmp="${OUTDIR}/cfg_${name}.o.tmp"
  local out="${OUTDIR}/cfg_${name}.o"
  # shellcheck disable=SC2086
  "$CC" $CFLAGS -c ini.c -o "$tmp" "$@" \
    -Dini_parse_stream="cfg_${name}_parse_stream" \
    -Dini_parse_file="cfg_${name}_parse_file" \
    -Dini_parse="cfg_${name}_parse" \
    -Dini_parse_string="cfg_${name}_parse_string" \
    -Dini_parse_string_length="cfg_${name}_parse_string_length"
  mv "$tmp" "$out"
}

compile multi_max_line -DINI_MAX_LINE=20
compile single -DINI_ALLOW_MULTILINE=0
compile disallow_inline_comments -DINI_ALLOW_INLINE_COMMENTS=0
compile stop_on_first_error -DINI_STOP_ON_FIRST_ERROR=1
compile handler_lineno -DINI_HANDLER_LINENO=1
compile string -DINI_MAX_LINE=20
compile heap -DINI_USE_STACK=0
compile heap_max_line -DINI_USE_STACK=0 -DINI_MAX_LINE=20 -DINI_INITIAL_ALLOC=20
compile heap_realloc -DINI_USE_STACK=0 -DINI_ALLOW_REALLOC=1 -DINI_INITIAL_ALLOC=5
compile heap_realloc_max_line -DINI_USE_STACK=0 -DINI_MAX_LINE=20 -DINI_ALLOW_REALLOC=1 -DINI_INITIAL_ALLOC=5
compile heap_string -DINI_USE_STACK=0 -DINI_MAX_LINE=20 -DINI_INITIAL_ALLOC=20
compile call_handler_on_new_section -DINI_CALL_HANDLER_ON_NEW_SECTION=1
compile allow_no_value -DINI_ALLOW_NO_VALUE=1
compile alloc -DINI_CUSTOM_ALLOCATOR=1 -DINI_USE_STACK=0 -DINI_ALLOW_REALLOC=1 -DINI_INITIAL_ALLOC=12

tmp="${OUTDIR}/ini.o.tmp"
# shellcheck disable=SC2086
"$CC" $CFLAGS -c ini.c -o "$tmp"
mv "$tmp" "${OUTDIR}/ini.o"
