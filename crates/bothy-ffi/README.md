# bothy-ffi: C ABI

A C interface to [Bothy](https://github.com/bothy-sync/bothy), for C, C++ and ROS 2 C++ code.

- **Header:** [`include/bothy.h`](include/bothy.h).
- **Library:** `libbothy_ffi` (shared or static).
- **Values:** documents and reports are JSON strings, so nothing in the ABI changes when the
  data model grows.

```c
#include "bothy.h"

BothyDb *db = bothy_open("{\"path\":\"robot.db\",\"app_id\":\"com.example.robot\","
                         "\"remote\":{\"folder\":\"/mnt/shared\"}}");
if (!db) { fprintf(stderr, "%s\n", bothy_last_error()); return 1; }
char *id = bothy_init_or_join(db);
bothy_put(db, "missions", "m1", "{\"goal\":\"dock\"}");
char *report = bothy_sync(db);
char *doc = bothy_get(db, "missions", "m1");
bothy_string_free(doc); bothy_string_free(report); bothy_string_free(id);
bothy_close(db);
```

- **Strings:** strings returned by `bothy_*` belong to the caller; free them with
  `bothy_string_free`.
- **Errors:** on failure a function returns `NULL` or `-1`. `bothy_last_error_kind()` and
  `bothy_last_error()` (per thread) say why. `bothy_get` returns `NULL` with `BOTHY_OK` for a
  missing document.
- **Configuration:** a JSON object with `path`, `app_id` and `remote`. `remote` is one of
  `{"folder": dir}`, `{"drive_folder": name}` or `{"drive_appdata": true}`. Optional keys:
  `password`, `encrypt`, `key_hex` (from `bothy_export_key`), `device_name` and
  `conflict_log_limit` (`null` turns the log off).
- **Google Drive:** remotes need `bothy_open_with_tokens`. Its callback fills a buffer with an
  access token; OAuth stays in your program.
- **Threads:** all functions are thread-safe; a panic inside the library is reported as
  `BOTHY_INTERNAL`.

## Build and test

```sh
cargo build --release -p bothy-ffi          # target/release/libbothy_ffi.{so,dylib,a}
crates/bothy-ffi/tests/run_smoke.sh         # compiles and runs tests/smoke.c
cbindgen --config crates/bothy-ffi/cbindgen.toml --crate bothy-ffi --output crates/bothy-ffi/include/bothy.h
```

This is the start of the C ABI (0.x): functions may still be added or changed.
