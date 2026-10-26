---
title: C and C++
description: A C interface for C, C++ and ROS 2 C++ nodes.
---

[`crates/bothy-ffi`](https://github.com/bothy-sync/bothy/tree/main/crates/bothy-ffi) builds
`libbothy_ffi` (shared or static) with the header `include/bothy.h`. Documents and reports cross
the boundary as JSON strings.

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
- **Errors:** on failure, a function returns `NULL` or `-1`, and `bothy_last_error_kind()` and
  `bothy_last_error()` (per thread) say why.
- **Remotes:** `remote` is `{"folder": dir}`, `{"drive_folder": name}` or
  `{"drive_appdata": true}`. Drive remotes need `bothy_open_with_tokens` with a callback that
  fills in an access token, so OAuth stays in your program.
- **Threads:** all functions are thread-safe.

```sh
cargo build --release -p bothy-ffi   # target/release/libbothy_ffi.{so,dylib,a}
```

The C ABI is early (0.x) and not published to a package registry; build it from the repository.
