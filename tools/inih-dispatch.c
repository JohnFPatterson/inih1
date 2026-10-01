/* Run every compiled inih oracle variant and print the parse section. */

#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/wait.h>
#include <unistd.h>

static const char *const VARIANTS[] = {
    "default",
    "max_line_20",
    "no_multiline",
    "no_inline_comments",
    "stop_on_first_error",
    "handler_lineno",
    "heap",
    "heap_max_line",
    "heap_realloc",
    "heap_realloc_max_line",
    "call_handler_on_new_section",
    "allow_no_value",
    "custom_alloc",
};

static int run_variant(const char *name, const char *fixture)
{
    char path[256];
    pid_t pid;
    int status;
    int n = snprintf(path, sizeof path, "build/oracle-%s", name);
    if (n < 0 || (size_t)n >= sizeof path) {
        fprintf(stderr, "variant path too long\n");
        return 1;
    }
    pid = fork();
    if (pid < 0) {
        perror("fork");
        return 1;
    }
    if (pid == 0) {
        execl(path, path, fixture, (char *)NULL);
        perror(path);
        _exit(127);
    }
    if (waitpid(pid, &status, 0) < 0) {
        perror("waitpid");
        return 1;
    }
    if (WIFEXITED(status)) {
        return WEXITSTATUS(status);
    }
    return 1;
}

int main(int argc, char **argv)
{
    const char *fixture = NULL;
    int show = 0;
    size_t i;
    int saw_sections = 0;
    int want_parse = 0;

    for (i = 1; (int)i < argc; i++) {
        if (strcmp(argv[i], "--sections") == 0) {
            const char *list;
            const char *p;
            if ((int)i + 1 >= argc) {
                fprintf(stderr, "missing --sections value\n");
                return 2;
            }
            list = argv[++i];
            saw_sections = 1;
            p = list;
            while (*p) {
                const char *comma = strchr(p, ',');
                size_t n = comma ? (size_t)(comma - p) : strlen(p);
                if (n == 5 && memcmp(p, "parse", 5) == 0) {
                    want_parse = 1;
                } else if (n != 0) {
                    fprintf(stderr, "unknown section\n");
                    return 2;
                }
                if (!comma) {
                    break;
                }
                p = comma + 1;
            }
        } else if (!fixture) {
            fixture = argv[i];
        } else {
            fprintf(stderr, "usage: inih-oracle [--sections parse] <fixture>\n");
            return 2;
        }
    }
    if (!fixture) {
        fprintf(stderr, "usage: inih-oracle [--sections parse] <fixture>\n");
        return 2;
    }
    show = saw_sections ? want_parse : 1;
    if (!show) {
        return 0;
    }
    if (fputs("== parse ==\n", stdout) == EOF) {
        return 1;
    }
    fflush(stdout);
    for (i = 0; i < sizeof VARIANTS / sizeof VARIANTS[0]; i++) {
        int rc = run_variant(VARIANTS[i], fixture);
        if (rc != 0) {
            return rc;
        }
    }
    return 0;
}
