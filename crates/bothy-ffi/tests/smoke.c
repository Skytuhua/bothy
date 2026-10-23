/* Smoke test of the C ABI: two devices share a folder remote. Run with tests/run_smoke.sh. */
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

#include "bothy.h"

#define CHECK(cond)                                                                 \
    do {                                                                            \
        if (!(cond)) {                                                              \
            fprintf(stderr, "%s:%d: %s failed (last error %d: %s)\n", __FILE__,     \
                    __LINE__, #cond, bothy_last_error_kind(), bothy_last_error());  \
            exit(1);                                                                \
        }                                                                           \
    } while (0)

static BothyDb *open_device(const char *dir, const char *name, const char *secret) {
    char cfg[2048];
    snprintf(cfg, sizeof cfg,
             "{\"path\":\"%s/%s.db\",\"app_id\":\"dev.bothy.ctest\",\"device_name\":\"%s\","
             "\"remote\":{\"folder\":\"%s/remote\"}%s}",
             dir, name, name, dir, secret);
    return bothy_open(cfg);
}

int main(int argc, char **argv) {
    CHECK(argc == 2);
    const char *dir = argv[1];
    printf("bothy %s\n", bothy_version());

    BothyDb *a = open_device(dir, "a", ",\"encrypt\":true,\"password\":\"pw\"");
    CHECK(a != NULL);
    char *id_a = bothy_init_or_join(a);
    CHECK(id_a != NULL && strlen(id_a) == 32);
    CHECK(bothy_put(a, "missions", "m1", "{\"goal\":\"dock\",\"speed\":0.5}") == 1);
    CHECK(bothy_put(a, "missions", "m1", "{\"goal\":\"dock\",\"speed\":0.5}") == 0);
    CHECK(bothy_patch(a, "missions", "m1", "{\"state\":\"new\"}") == 1);
    char *report = bothy_sync(a);
    CHECK(report != NULL && strstr(report, "\"pushed_ops\":2") != NULL);
    bothy_string_free(report);
    char *key = bothy_export_key(a);
    CHECK(key != NULL && strlen(key) == 64);

    /* A wrong password is refused with a clear kind. */
    BothyDb *wrong = open_device(dir, "w", ",\"password\":\"nope\"");
    CHECK(wrong != NULL);
    CHECK(bothy_init_or_join(wrong) == NULL);
    CHECK(bothy_last_error_kind() == BOTHY_WRONG_PASSWORD);
    bothy_close(wrong);

    /* A second device joins with the kept key. */
    char secret[128];
    snprintf(secret, sizeof secret, ",\"password\":\"pw\"");
    BothyDb *b = open_device(dir, "b", secret);
    CHECK(b != NULL);
    char *id_b = bothy_init_or_join(b);
    CHECK(id_b != NULL && strcmp(id_a, id_b) == 0);
    char *doc = bothy_get(b, "missions", "m1");
    CHECK(doc != NULL && strstr(doc, "\"goal\":\"dock\"") && strstr(doc, "\"state\":\"new\""));
    bothy_string_free(doc);
    CHECK(bothy_get(b, "missions", "missing") == NULL && bothy_last_error_kind() == BOTHY_OK);

    CHECK(bothy_put(b, "telemetry_logs", "t1", "{\"temp_c\":41.5}") == 1);
    bothy_string_free(bothy_sync(b));
    bothy_string_free(bothy_sync(a));
    char *list = bothy_list(a, "telemetry_logs", 10, NULL);
    CHECK(list != NULL && strstr(list, "\"id\":\"t1\"") && strstr(list, "41.5"));
    bothy_string_free(list);
    CHECK(bothy_delete(a, "telemetry_logs", "t1") == 1);
    CHECK(bothy_delete(a, "telemetry_logs", "t1") == 0);

    /* Errors: bad names, bad JSON, NULL arguments. */
    CHECK(bothy_put(a, "Bad Name", "x", "{}") == -1);
    CHECK(bothy_last_error_kind() == BOTHY_INVALID_INPUT && strlen(bothy_last_error()) > 0);
    CHECK(bothy_put(a, "ok", "x", "[1]") == -1);
    CHECK(bothy_put(NULL, "ok", "x", "{}") == -1);
    CHECK(bothy_open("{\"path\":\"x\"}") == NULL);

    /* Reopen with the key instead of the password. */
    bothy_close(a);
    snprintf(secret, sizeof secret, ",\"key_hex\":\"%s\"", key);
    a = open_device(dir, "a", secret);
    CHECK(a != NULL);
    bothy_string_free(bothy_sync(a));

    bothy_string_free(key);
    bothy_string_free(id_a);
    bothy_string_free(id_b);
    bothy_close(a);
    bothy_close(b);
    printf("ok\n");
    return 0;
}
