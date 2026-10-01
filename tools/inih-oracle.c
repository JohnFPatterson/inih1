/* Differential oracle for inih. Public ini.h symbols are the default ABI.
   Every other tests/unittest.sh configuration is a renamed translation unit.
   See tools/DRIVER_FORMAT.md. */

#include <locale.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

#include "ini.h"

static int g_fail_after = -1;
static int g_alloc_i = 0;

void *ini_malloc(size_t size) {
    printf("A\tmalloc\t%d\n", (int)size);
    if (g_fail_after >= 0 && g_alloc_i == g_fail_after) {
        g_alloc_i++;
        return NULL;
    }
    g_alloc_i++;
    return malloc(size);
}

void ini_free(void *ptr) {
    printf("A\tfree\n");
    free(ptr);
}

void *ini_realloc(void *ptr, size_t size) {
    printf("A\trealloc\t%d\n", (int)size);
    if (g_fail_after >= 0 && g_alloc_i == g_fail_after) {
        g_alloc_i++;
        return NULL;
    }
    g_alloc_i++;
    return realloc(ptr, size);
}

typedef int (*handler4)(void *, const char *, const char *, const char *);
typedef int (*handler5)(void *, const char *, const char *, const char *, int);
typedef int (*parse4)(const char *, handler4, void *);
typedef int (*parse_file4)(FILE *, handler4, void *);
typedef int (*parse_stream4)(ini_reader, void *, handler4, void *);
typedef int (*parse_string4)(const char *, handler4, void *);
typedef int (*parse_string_length4)(const char *, size_t, handler4, void *);

#define DECL4(name)                                                            \
    int cfg_##name##_parse(const char *, handler4, void *);                   \
    int cfg_##name##_parse_file(FILE *, handler4, void *);                    \
    int cfg_##name##_parse_stream(ini_reader, void *, handler4, void *);      \
    int cfg_##name##_parse_string(const char *, handler4, void *);            \
    int cfg_##name##_parse_string_length(const char *, size_t, handler4, void *)

DECL4(multi_max_line);
DECL4(single);
DECL4(disallow_inline_comments);
DECL4(stop_on_first_error);
DECL4(string);
DECL4(heap);
DECL4(heap_max_line);
DECL4(heap_realloc);
DECL4(heap_realloc_max_line);
DECL4(heap_string);
DECL4(call_handler_on_new_section);
DECL4(allow_no_value);
DECL4(alloc);

int cfg_handler_lineno_parse(const char *, handler5, void *);
int cfg_handler_lineno_parse_file(FILE *, handler5, void *);
int cfg_handler_lineno_parse_stream(ini_reader, void *, handler5, void *);
int cfg_handler_lineno_parse_string(const char *, handler5, void *);
int cfg_handler_lineno_parse_string_length(const char *, size_t, handler5, void *);

struct call_ctx {
    int id;
    int called;
    const char *config;
    const char *api;
};

struct read_ctx {
    FILE *fp;
    const char *config;
    const char *api;
};

struct bundle4 {
    const char *name;
    parse4 parse;
    parse_file4 parse_file;
    parse_stream4 parse_stream;
    parse_string4 parse_string;
    parse_string_length4 parse_string_length;
};

static void print_field(const char *s) {
    const unsigned char *p;
    if (!s) {
        fputs("\\N", stdout);
        return;
    }
    for (p = (const unsigned char *)s; *p; p++) {
        unsigned char c = *p;
        if (c == '\\') {
            fputs("\\\\", stdout);
        } else if (c == '\n') {
            fputs("\\n", stdout);
        } else if (c == '\r') {
            fputs("\\r", stdout);
        } else if (c == '\t') {
            fputs("\\t", stdout);
        } else if (c < 0x20 || c == 0x7f) {
            printf("\\x%02x", c);
        } else {
            fputc(c, stdout);
        }
    }
}

static void print_result(const struct call_ctx *ctx, int code) {
    printf("R\t%s\t%s\t%d\t", ctx->config, ctx->api, code);
    if (ctx->called) {
        printf("%d\n", ctx->id);
    } else {
        fputs("-\n", stdout);
    }
}

static int handler_rc(const char *name, const char *value) {
    if (name && value && strcmp(name, "user") == 0 && strcmp(value, "parse_error") == 0) {
        return 0;
    }
    return 1;
}

static void print_h(const struct call_ctx *ctx, int lineno, int rc, const char *section,
                    const char *name, const char *value) {
    fputs("H\t", stdout);
    fputs(ctx->config, stdout);
    fputc('\t', stdout);
    fputs(ctx->api, stdout);
    fputc('\t', stdout);
    if (lineno < 0) {
        fputc('-', stdout);
    } else {
        printf("%d", lineno);
    }
    printf("\t%d\t", rc);
    print_field(section);
    fputc('\t', stdout);
    print_field(name);
    fputc('\t', stdout);
    print_field(value);
    fputc('\n', stdout);
}

static int handle4(void *user, const char *section, const char *name, const char *value) {
    struct call_ctx *ctx = user;
    int rc = handler_rc(name, value);
    ctx->called = 1;
    print_h(ctx, -1, rc, section, name, value);
    return rc;
}

static int handle5(void *user, const char *section, const char *name, const char *value, int lineno) {
    struct call_ctx *ctx = user;
    int rc = handler_rc(name, value);
    ctx->called = 1;
    print_h(ctx, lineno, rc, section, name, value);
    return rc;
}

static char *trace_read(char *str, int num, void *stream) {
    struct read_ctx *ctx = stream;
    char *got = fgets(str, num, ctx->fp);
    printf("D\t%s\t%s\t%d\t", ctx->config, ctx->api, num);
    if (got) {
        print_field(got);
    } else {
        fputs("\\N", stdout);
    }
    fputc('\n', stdout);
    return got;
}

static int read_all(const char *path, char **out, size_t *out_len) {
    FILE *fp = fopen(path, "rb");
    long sz;
    char *buf;
    size_t n;
    if (!fp) {
        return -1;
    }
    if (fseek(fp, 0, SEEK_END) != 0) {
        fclose(fp);
        return -1;
    }
    sz = ftell(fp);
    if (sz < 0 || fseek(fp, 0, SEEK_SET) != 0) {
        fclose(fp);
        return -1;
    }
    buf = (char *)malloc((size_t)sz + 1);
    if (!buf) {
        fclose(fp);
        return -2;
    }
    n = fread(buf, 1, (size_t)sz, fp);
    if (n != (size_t)sz && ferror(fp)) {
        fclose(fp);
        free(buf);
        return -1;
    }
    fclose(fp);
    buf[n] = '\0';
    *out = buf;
    *out_len = n;
    return 0;
}

static void run4(const struct bundle4 *b, const char *path) {
    struct call_ctx ctx;
    struct read_ctx reader;
    char *buf = NULL;
    size_t len = 0;
    int io;
    FILE *fp;

    ctx.id = 100;
    ctx.config = b->name;

    printf("K\t%s\n", b->name);

    g_alloc_i = 0;
    ctx.called = 0;
    ctx.api = "parse";
    print_result(&ctx, b->parse(path, handle4, &ctx));

    g_alloc_i = 0;
    ctx.called = 0;
    ctx.api = "file";
    fp = fopen(path, "r");
    if (!fp) {
        print_result(&ctx, -1);
    } else {
        print_result(&ctx, b->parse_file(fp, handle4, &ctx));
        fclose(fp);
    }

    g_alloc_i = 0;
    ctx.called = 0;
    ctx.api = "stream";
    fp = fopen(path, "r");
    if (!fp) {
        print_result(&ctx, -1);
    } else {
        reader.fp = fp;
        reader.config = b->name;
        reader.api = "stream";
        print_result(&ctx, b->parse_stream(trace_read, &reader, handle4, &ctx));
        fclose(fp);
    }

    io = read_all(path, &buf, &len);
    g_alloc_i = 0;
    ctx.called = 0;
    ctx.api = "string";
    if (io != 0) {
        print_result(&ctx, io);
    } else {
        print_result(&ctx, b->parse_string(buf, handle4, &ctx));
    }

    g_alloc_i = 0;
    ctx.called = 0;
    ctx.api = "length";
    if (io != 0) {
        print_result(&ctx, io);
    } else {
        print_result(&ctx, b->parse_string_length(buf, len, handle4, &ctx));
    }
    free(buf);
}

static void run_lineno(const char *path) {
    struct call_ctx ctx;
    struct read_ctx reader;
    char *buf = NULL;
    size_t len = 0;
    int io;
    FILE *fp;

    ctx.id = 100;
    ctx.config = "handler_lineno";
    printf("K\t%s\n", ctx.config);

    g_alloc_i = 0;
    ctx.called = 0;
    ctx.api = "parse";
    print_result(&ctx, cfg_handler_lineno_parse(path, handle5, &ctx));

    g_alloc_i = 0;
    ctx.called = 0;
    ctx.api = "file";
    fp = fopen(path, "r");
    if (!fp) {
        print_result(&ctx, -1);
    } else {
        print_result(&ctx, cfg_handler_lineno_parse_file(fp, handle5, &ctx));
        fclose(fp);
    }

    g_alloc_i = 0;
    ctx.called = 0;
    ctx.api = "stream";
    fp = fopen(path, "r");
    if (!fp) {
        print_result(&ctx, -1);
    } else {
        reader.fp = fp;
        reader.config = ctx.config;
        reader.api = "stream";
        print_result(&ctx, cfg_handler_lineno_parse_stream(trace_read, &reader, handle5, &ctx));
        fclose(fp);
    }

    io = read_all(path, &buf, &len);
    g_alloc_i = 0;
    ctx.called = 0;
    ctx.api = "string";
    if (io != 0) {
        print_result(&ctx, io);
    } else {
        print_result(&ctx, cfg_handler_lineno_parse_string(buf, handle5, &ctx));
    }

    g_alloc_i = 0;
    ctx.called = 0;
    ctx.api = "length";
    if (io != 0) {
        print_result(&ctx, io);
    } else {
        print_result(&ctx, cfg_handler_lineno_parse_string_length(buf, len, handle5, &ctx));
    }
    free(buf);
}

static void run_embedded(const struct bundle4 *b, const char *case_name, const char *text) {
    struct call_ctx ctx;
    printf("C\t%s\t%s\n", b->name, case_name);
    ctx.id = 100;
    ctx.called = 0;
    ctx.config = b->name;
    ctx.api = "string";
    g_alloc_i = 0;
    print_result(&ctx, b->parse_string(text, handle4, &ctx));
}

static int want(const char *list, const char *name) {
    size_t n = strlen(name);
    const char *p = list;
    while (*p) {
        const char *comma = strchr(p, ',');
        size_t len = comma ? (size_t)(comma - p) : strlen(p);
        if (len == n && memcmp(p, name, n) == 0) {
            return 1;
        }
        if (!comma) {
            break;
        }
        p = comma + 1;
    }
    return 0;
}

static int sections_known(const char *list) {
    const char *p = list;
    if (!list || !*list) {
        return 0;
    }
    while (*p) {
        const char *comma = strchr(p, ',');
        size_t len = comma ? (size_t)(comma - p) : strlen(p);
        char name[32];
        if (len == 0 || len >= sizeof(name)) {
            return 0;
        }
        memcpy(name, p, len);
        name[len] = '\0';
        if (strcmp(name, "file") != 0 && strcmp(name, "string") != 0 && strcmp(name, "alloc") != 0 &&
            strcmp(name, "allocfile") != 0) {
            return 0;
        }
        if (!comma) {
            break;
        }
        p = comma + 1;
    }
    return 1;
}

static const struct bundle4 BUNDLES[] = {
    {"multi_max_line", cfg_multi_max_line_parse, cfg_multi_max_line_parse_file,
     cfg_multi_max_line_parse_stream, cfg_multi_max_line_parse_string,
     cfg_multi_max_line_parse_string_length},
    {"single", cfg_single_parse, cfg_single_parse_file, cfg_single_parse_stream, cfg_single_parse_string,
     cfg_single_parse_string_length},
    {"disallow_inline_comments", cfg_disallow_inline_comments_parse, cfg_disallow_inline_comments_parse_file,
     cfg_disallow_inline_comments_parse_stream, cfg_disallow_inline_comments_parse_string,
     cfg_disallow_inline_comments_parse_string_length},
    {"stop_on_first_error", cfg_stop_on_first_error_parse, cfg_stop_on_first_error_parse_file,
     cfg_stop_on_first_error_parse_stream, cfg_stop_on_first_error_parse_string,
     cfg_stop_on_first_error_parse_string_length},
    {"heap", cfg_heap_parse, cfg_heap_parse_file, cfg_heap_parse_stream, cfg_heap_parse_string,
     cfg_heap_parse_string_length},
    {"heap_max_line", cfg_heap_max_line_parse, cfg_heap_max_line_parse_file, cfg_heap_max_line_parse_stream,
     cfg_heap_max_line_parse_string, cfg_heap_max_line_parse_string_length},
    {"heap_realloc", cfg_heap_realloc_parse, cfg_heap_realloc_parse_file, cfg_heap_realloc_parse_stream,
     cfg_heap_realloc_parse_string, cfg_heap_realloc_parse_string_length},
    {"heap_realloc_max_line", cfg_heap_realloc_max_line_parse, cfg_heap_realloc_max_line_parse_file,
     cfg_heap_realloc_max_line_parse_stream, cfg_heap_realloc_max_line_parse_string,
     cfg_heap_realloc_max_line_parse_string_length},
    {"call_handler_on_new_section", cfg_call_handler_on_new_section_parse,
     cfg_call_handler_on_new_section_parse_file, cfg_call_handler_on_new_section_parse_stream,
     cfg_call_handler_on_new_section_parse_string, cfg_call_handler_on_new_section_parse_string_length},
    {"allow_no_value", cfg_allow_no_value_parse, cfg_allow_no_value_parse_file, cfg_allow_no_value_parse_stream,
     cfg_allow_no_value_parse_string, cfg_allow_no_value_parse_string_length},
    {"alloc", cfg_alloc_parse, cfg_alloc_parse_file, cfg_alloc_parse_stream, cfg_alloc_parse_string,
     cfg_alloc_parse_string_length},
};

static const struct bundle4 STRING_BUNDLES[] = {
    {"string", cfg_string_parse, cfg_string_parse_file, cfg_string_parse_stream, cfg_string_parse_string,
     cfg_string_parse_string_length},
    {"heap_string", cfg_heap_string_parse, cfg_heap_string_parse_file, cfg_heap_string_parse_stream,
     cfg_heap_string_parse_string, cfg_heap_string_parse_string_length},
};

static void run_file_configs(const char *path, int alloc_only) {
    struct bundle4 multi;
    size_t i;
    multi.name = "multi";
    multi.parse = ini_parse;
    multi.parse_file = ini_parse_file;
    multi.parse_stream = ini_parse_stream;
    multi.parse_string = ini_parse_string;
    multi.parse_string_length = ini_parse_string_length;
    if (!alloc_only) {
        run4(&multi, path);
        for (i = 0; i < sizeof(BUNDLES) / sizeof(BUNDLES[0]); i++) {
            if (strcmp(BUNDLES[i].name, "alloc") == 0) {
                continue;
            }
            if (strcmp(BUNDLES[i].name, "heap") == 0) {
                run_lineno(path);
            }
            run4(&BUNDLES[i], path);
        }
    }
    for (i = 0; i < sizeof(BUNDLES) / sizeof(BUNDLES[0]); i++) {
        if (strcmp(BUNDLES[i].name, "alloc") == 0) {
            run4(&BUNDLES[i], path);
        }
    }
}

static void run_string_block(void) {
    static const char *cases[][2] = {
        {"empty string", ""},
        {"basic", "[section]\nfoo = bar\nbazz = buzz quxx"},
        {"crlf", "[section]\r\nhello = world\r\nforty_two = 42\r\n"},
        {"long line", "[sec]\nfoo = 01234567890123456789\nbar=4321\n"},
        {"long continued", "[sec]\nfoo = 0123456789012bix=1234\n"},
        {"error", "[s]\na=1\nb\nc=3"},
    };
    size_t b, c;
    for (b = 0; b < sizeof(STRING_BUNDLES) / sizeof(STRING_BUNDLES[0]); b++) {
        printf("K\t%s\n", STRING_BUNDLES[b].name);
        for (c = 0; c < sizeof(cases) / sizeof(cases[0]); c++) {
            run_embedded(&STRING_BUNDLES[b], cases[c][0], cases[c][1]);
        }
    }
}

static void run_alloc_block(void) {
    size_t i;
    for (i = 0; i < sizeof(BUNDLES) / sizeof(BUNDLES[0]); i++) {
        if (strcmp(BUNDLES[i].name, "alloc") != 0) {
            continue;
        }
        printf("K\t%s\n", BUNDLES[i].name);
        run_embedded(&BUNDLES[i], "basic", "[section]\nfoo = bar\nbazz = buzz quxx");
    }
}

static void usage(void) {
    fputs("usage: inih-oracle [--sections file,string,alloc,allocfile] [--alloc-fail N] <fixture>\n",
          stderr);
}

int main(int argc, char **argv) {
    const char *sections = "file,string,alloc";
    const char *input = NULL;
    int i;

    setlocale(LC_ALL, "C");
    for (i = 1; i < argc; i++) {
        if (strcmp(argv[i], "--sections") == 0) {
            if (i + 1 >= argc) {
                usage();
                return 2;
            }
            sections = argv[++i];
        } else if (strcmp(argv[i], "--alloc-fail") == 0) {
            char *end = NULL;
            long n;
            if (i + 1 >= argc) {
                usage();
                return 2;
            }
            n = strtol(argv[++i], &end, 10);
            if (!end || *end || n < 0 || n > 1000000) {
                usage();
                return 2;
            }
            g_fail_after = (int)n;
        } else if (strcmp(argv[i], "--help") == 0) {
            usage();
            return 2;
        } else if (argv[i][0] == '-') {
            usage();
            return 2;
        } else if (input) {
            usage();
            return 2;
        } else {
            input = argv[i];
        }
    }
    if (!input || !sections_known(sections)) {
        usage();
        return 2;
    }

    if (want(sections, "file")) {
        fputs("S\tfile\n", stdout);
        run_file_configs(input, 0);
    }
    if (want(sections, "allocfile")) {
        fputs("S\tallocfile\n", stdout);
        run_file_configs(input, 1);
    }
    if (want(sections, "string")) {
        fputs("S\tstring\n", stdout);
        run_string_block();
    }
    if (want(sections, "alloc")) {
        fputs("S\talloc\n", stdout);
        run_alloc_block();
    }
    return 0;
}
