/* Differential oracle for inih. Public header only; compile with ini.c or libinih_ffi.a. */

#include "ini.h"

#include <stdio.h>
#include <stdlib.h>
#include <string.h>

#ifndef INI_ORACLE_CONFIG
#define INI_ORACLE_CONFIG "default"
#endif

static char *trace_buf;
static size_t trace_len;
static size_t trace_cap;

static int trace_append(const char *s, size_t n)
{
    if (trace_len + n + 1 > trace_cap) {
        size_t cap = trace_cap ? trace_cap : 256;
        while (cap < trace_len + n + 1) {
            if (cap > (size_t)-1 / 2) {
                return -1;
            }
            cap *= 2;
        }
        char *grown = (char *)realloc(trace_buf, cap);
        if (!grown) {
            return -1;
        }
        trace_buf = grown;
        trace_cap = cap;
    }
    memcpy(trace_buf + trace_len, s, n);
    trace_len += n;
    trace_buf[trace_len] = '\0';
    return 0;
}

static int trace_cstr(const char *s)
{
    return trace_append(s, strlen(s));
}

static int quote_field(const char *s)
{
    if (!s) {
        return trace_cstr("null");
    }
    if (trace_cstr("\"")) {
        return -1;
    }
    for (const unsigned char *p = (const unsigned char *)s; *p; p++) {
        char tmp[8];
        int n;
        if (*p == '\\' || *p == '"') {
            tmp[0] = '\\';
            tmp[1] = (char)*p;
            if (trace_append(tmp, 2)) {
                return -1;
            }
        } else if (*p == '\n') {
            if (trace_cstr("\\n")) {
                return -1;
            }
        } else if (*p == '\r') {
            if (trace_cstr("\\r")) {
                return -1;
            }
        } else if (*p == '\t') {
            if (trace_cstr("\\t")) {
                return -1;
            }
        } else if (*p < 0x20 || *p == 0x7f) {
            n = sprintf(tmp, "\\x%02x", *p);
            if (n < 0 || trace_append(tmp, (size_t)n)) {
                return -1;
            }
        } else {
            tmp[0] = (char)*p;
            if (trace_append(tmp, 1)) {
                return -1;
            }
        }
    }
    return trace_cstr("\"");
}

#if INI_CUSTOM_ALLOCATOR
void *ini_malloc(size_t size)
{
    return malloc(size);
}

void ini_free(void *ptr)
{
    free(ptr);
}

void *ini_realloc(void *ptr, size_t size)
{
    return realloc(ptr, size);
}
#endif

static void trace_or_exit(int failed)
{
    if (failed) {
        fprintf(stderr, "oracle trace allocation failed\n");
        exit(1);
    }
}

#if INI_HANDLER_LINENO
static int handler(void *user, const char *section, const char *name, const char *value, int lineno)
#else
static int handler(void *user, const char *section, const char *name, const char *value)
#endif
{
    char lineno_buf[32];
    (void)user;
    trace_or_exit(quote_field(section));
    trace_or_exit(trace_cstr("\t"));
    trace_or_exit(quote_field(name));
    trace_or_exit(trace_cstr("\t"));
    trace_or_exit(quote_field(value));
#if INI_HANDLER_LINENO
    sprintf(lineno_buf, "\t%d", lineno);
    trace_or_exit(trace_cstr(lineno_buf));
#else
    (void)lineno_buf;
#endif
    trace_or_exit(trace_cstr("\n"));
    return 1;
}

static int emit(const char *api, int rc)
{
    if (printf("config %s api %s\nrc %d\n", INI_ORACLE_CONFIG, api, rc) < 0) {
        return -1;
    }
    if (trace_len && fwrite(trace_buf, 1, trace_len, stdout) != trace_len) {
        return -1;
    }
    trace_len = 0;
    return 0;
}

static int load_file(const char *path, char **out, size_t *out_len)
{
    FILE *f = fopen(path, "rb");
    long sz;
    char *buf;
    size_t n;
    if (!f) {
        return -1;
    }
    if (fseek(f, 0, SEEK_END) != 0) {
        fclose(f);
        return -1;
    }
    sz = ftell(f);
    if (sz < 0) {
        fclose(f);
        return -1;
    }
    if (fseek(f, 0, SEEK_SET) != 0) {
        fclose(f);
        return -1;
    }
    buf = (char *)malloc((size_t)sz + 1);
    if (!buf) {
        fclose(f);
        return -1;
    }
    n = fread(buf, 1, (size_t)sz, f);
    fclose(f);
    if (n != (size_t)sz) {
        free(buf);
        return -1;
    }
    buf[n] = '\0';
    *out = buf;
    *out_len = n;
    return 0;
}

static int run_open_failure(void)
{
    const char *apis[] = {"file", "fileptr", "stream", "string", "length"};
    size_t i;
    for (i = 0; i < sizeof apis / sizeof apis[0]; i++) {
        if (emit(apis[i], -1)) {
            return -1;
        }
    }
    return 0;
}

int main(int argc, char **argv)
{
    const char *path;
    char *data = NULL;
    size_t len = 0;
    FILE *f;
    int rc;

    if (argc != 2) {
        fprintf(stderr, "usage: inih-oracle <fixture>\n");
        return 2;
    }
    path = argv[1];

    f = fopen(path, "r");
    if (!f) {
        free(trace_buf);
        return run_open_failure() ? 1 : 0;
    }

    rc = ini_parse(path, handler, NULL);
    if (emit("file", rc)) {
        fclose(f);
        free(trace_buf);
        return 1;
    }

    rewind(f);
    rc = ini_parse_file(f, handler, NULL);
    if (emit("fileptr", rc)) {
        fclose(f);
        free(trace_buf);
        return 1;
    }

    rewind(f);
    rc = ini_parse_stream((ini_reader)fgets, f, handler, NULL);
    if (emit("stream", rc)) {
        fclose(f);
        free(trace_buf);
        return 1;
    }
    fclose(f);

    if (load_file(path, &data, &len)) {
        free(trace_buf);
        return 1;
    }

    rc = ini_parse_string(data, handler, NULL);
    if (emit("string", rc)) {
        free(data);
        free(trace_buf);
        return 1;
    }

    rc = ini_parse_string_length(data, len, handler, NULL);
    if (emit("length", rc)) {
        free(data);
        free(trace_buf);
        return 1;
    }

    free(data);
    free(trace_buf);
    return 0;
}
