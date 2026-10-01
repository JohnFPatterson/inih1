/* Hook trace for ini_handler and ini_malloc/ini_free/ini_realloc.
 * Public header only. Default builds allocate nothing. custom_alloc builds
 * log M/R/F. --fail N fails the Nth malloc/realloc of each API call (1-based).
 */

#include "ini.h"

#include <stdio.h>
#include <stdlib.h>
#include <string.h>

#ifndef INI_HOOK_CONFIG
#define INI_HOOK_CONFIG "default"
#endif

static int fail_n;
static int alloc_i;
#if INI_CUSTOM_ALLOCATOR
static size_t live_size;
#endif

static void reset_alloc(void)
{
    alloc_i = 0;
}

#if INI_CUSTOM_ALLOCATOR
void *ini_malloc(size_t size)
{
    alloc_i++;
    printf("M %zu\n", size);
    if (fail_n > 0 && alloc_i == fail_n) {
        return NULL;
    }
    void *p = malloc(size);
    if (p) {
        live_size = size;
    }
    return p;
}

void ini_free(void *ptr)
{
    printf("F %zu\n", live_size);
    free(ptr);
}

void *ini_realloc(void *ptr, size_t size)
{
    alloc_i++;
    printf("R %zu\n", size);
    if (fail_n > 0 && alloc_i == fail_n) {
        return NULL;
    }
    void *p = realloc(ptr, size);
    if (p) {
        live_size = size;
    }
    return p;
}
#endif

static void quote(const char *s)
{
    const unsigned char *p;
    if (!s) {
        fputs("null", stdout);
        return;
    }
    fputc('"', stdout);
    for (p = (const unsigned char *)s; *p; p++) {
        if (*p == '\\' || *p == '"') {
            fputc('\\', stdout);
            fputc((int)*p, stdout);
        } else if (*p == '\n') {
            fputs("\\n", stdout);
        } else if (*p == '\r') {
            fputs("\\r", stdout);
        } else if (*p == '\t') {
            fputs("\\t", stdout);
        } else if (*p < 0x20 || *p == 0x7f) {
            printf("\\x%02x", *p);
        } else {
            fputc((int)*p, stdout);
        }
    }
    fputc('"', stdout);
}

#if INI_HANDLER_LINENO
static int handler(void *user, const char *section, const char *name, const char *value, int lineno)
#else
static int handler(void *user, const char *section, const char *name, const char *value)
#endif
{
    (void)user;
#if INI_HANDLER_LINENO
    (void)lineno;
#endif
    fputs("H\t", stdout);
    quote(section);
    fputc('\t', stdout);
    quote(name);
    fputc('\t', stdout);
    quote(value);
    fputc('\n', stdout);
    return 1;
}

static void run_string(const char *label, const char *text)
{
    int rc;
    reset_alloc();
    printf("api string %s\n", label);
    rc = ini_parse_string(text, handler, NULL);
    printf("rc %d\n", rc);
    reset_alloc();
    printf("api length %s\n", label);
    rc = ini_parse_string_length(text, strlen(text), handler, NULL);
    printf("rc %d\n", rc);
}

static char *load_file(const char *path, size_t *len)
{
    FILE *f = fopen(path, "rb");
    long sz;
    char *buf;
    size_t n;
    if (!f) {
        return NULL;
    }
    if (fseek(f, 0, SEEK_END) != 0 || (sz = ftell(f)) < 0 || fseek(f, 0, SEEK_SET) != 0) {
        fclose(f);
        return NULL;
    }
    buf = (char *)malloc((size_t)sz + 1);
    if (!buf) {
        fclose(f);
        return NULL;
    }
    n = fread(buf, 1, (size_t)sz, f);
    fclose(f);
    if (n != (size_t)sz) {
        free(buf);
        return NULL;
    }
    buf[n] = '\0';
    *len = n;
    return buf;
}

static void run_file(const char *path)
{
    FILE *f;
    char *data;
    size_t len = 0;
    int rc;

    reset_alloc();
    printf("api file %s\n", path);
    rc = ini_parse(path, handler, NULL);
    printf("rc %d\n", rc);

    f = fopen(path, "r");
    reset_alloc();
    printf("api fileptr %s\n", path);
    if (!f) {
        printf("rc -1\n");
    } else {
        rc = ini_parse_file(f, handler, NULL);
        printf("rc %d\n", rc);
        fclose(f);
    }

    f = fopen(path, "r");
    reset_alloc();
    printf("api stream %s\n", path);
    if (!f) {
        printf("rc -1\n");
    } else {
        rc = ini_parse_stream((ini_reader)fgets, f, handler, NULL);
        printf("rc %d\n", rc);
        fclose(f);
    }

    data = load_file(path, &len);
    reset_alloc();
    printf("api string %s\n", path);
    if (!data) {
        printf("rc -1\n");
    } else {
        rc = ini_parse_string(data, handler, NULL);
        printf("rc %d\n", rc);
    }
    reset_alloc();
    printf("api length %s\n", path);
    if (!data) {
        printf("rc -1\n");
    } else {
        rc = ini_parse_string_length(data, len, handler, NULL);
        printf("rc %d\n", rc);
        free(data);
    }
}

static void run_all(void)
{
    run_string("empty", "");
    run_string("basic", "[section]\nfoo = bar\nbazz = buzz quxx");
    run_string("sections", "[a]\nx=1\n\n[b]\ny=2\n");
    run_file("tests/normal.ini");
    run_file("tests/bad_comment.ini");
    run_file("tests/bom.ini");
}

int main(int argc, char **argv)
{
    int i;
    for (i = 1; i < argc; i++) {
        if (strcmp(argv[i], "--fail") == 0 && i + 1 < argc) {
            fail_n = atoi(argv[++i]);
        } else {
            fprintf(stderr, "usage: hook-trace [--fail N]\n");
            return 2;
        }
    }
    printf("config %s\nfail %d\n", INI_HOOK_CONFIG, fail_n);
    run_all();
    return 0;
}
