/* Public-API hook trace: handler and reader callbacks.
   Link against ini.c or against libinih_ffi.a; stdout must match.
   See tools/DRIVER_FORMAT.md. Custom-allocator failures are traced by
   `inih-oracle --sections alloc --alloc-fail N` versus the Rust driver,
   because the default ABI does not expose ini_malloc. */

#include <locale.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

#include "ini.h"

struct call_ctx {
    int called;
    const char *api;
};

struct read_ctx {
    FILE *fp;
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

static int handler_rc(const char *name, const char *value) {
    if (name && value && strcmp(name, "user") == 0 && strcmp(value, "parse_error") == 0) {
        return 0;
    }
    return 1;
}

static int handle(void *user, const char *section, const char *name, const char *value) {
    struct call_ctx *ctx = user;
    int rc = handler_rc(name, value);
    ctx->called = 1;
    fputs("H\tdefault\t", stdout);
    fputs(ctx->api, stdout);
    printf("\t-\t%d\t", rc);
    print_field(section);
    fputc('\t', stdout);
    print_field(name);
    fputc('\t', stdout);
    print_field(value);
    fputc('\n', stdout);
    return rc;
}

static void print_result(const struct call_ctx *ctx, int code) {
    printf("R\tdefault\t%s\t%d\t", ctx->api, code);
    if (ctx->called) {
        fputs("100\n", stdout);
    } else {
        fputs("-\n", stdout);
    }
}

static char *trace_read(char *str, int num, void *stream) {
    struct read_ctx *ctx = stream;
    char *got = fgets(str, num, ctx->fp);
    printf("D\tdefault\tstream\t%d\t", num);
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

static void run_file(const char *path) {
    struct call_ctx ctx;
    struct read_ctx reader;
    char *buf = NULL;
    size_t len = 0;
    int io;
    FILE *fp;

    ctx.api = "parse";
    ctx.called = 0;
    print_result(&ctx, ini_parse(path, handle, &ctx));

    ctx.api = "file";
    ctx.called = 0;
    fp = fopen(path, "r");
    if (!fp) {
        print_result(&ctx, -1);
    } else {
        print_result(&ctx, ini_parse_file(fp, handle, &ctx));
        fclose(fp);
    }

    ctx.api = "stream";
    ctx.called = 0;
    fp = fopen(path, "r");
    if (!fp) {
        print_result(&ctx, -1);
    } else {
        reader.fp = fp;
        print_result(&ctx, ini_parse_stream(trace_read, &reader, handle, &ctx));
        fclose(fp);
    }

    io = read_all(path, &buf, &len);
    ctx.api = "string";
    ctx.called = 0;
    if (io != 0) {
        print_result(&ctx, io);
    } else {
        print_result(&ctx, ini_parse_string(buf, handle, &ctx));
    }

    ctx.api = "length";
    ctx.called = 0;
    if (io != 0) {
        print_result(&ctx, io);
    } else {
        print_result(&ctx, ini_parse_string_length(buf, len, handle, &ctx));
    }
    free(buf);
}

static void run_case(const char *name, const char *text) {
    struct call_ctx ctx;
    printf("C\tdefault\t%s\n", name);
    ctx.api = "string";
    ctx.called = 0;
    print_result(&ctx, ini_parse_string(text, handle, &ctx));
}

int main(int argc, char **argv) {
    setlocale(LC_ALL, "C");
    if (argc != 2) {
        fputs("usage: hook-trace <fixture>\n", stderr);
        return 2;
    }
    fputs("S\thook\n", stdout);
    run_file(argv[1]);
    run_case("empty string", "");
    run_case("basic", "[section]\nfoo = bar\nbazz = buzz quxx");
    run_case("user error", "[s]\nuser=parse_error\nok=1\n");
    return 0;
}
