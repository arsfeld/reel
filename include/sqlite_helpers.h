#ifndef SQLITE_HELPERS_H
#define SQLITE_HELPERS_H

#include <sqlite3.h>

// Wrapper to call sqlite3_bind_text with SQLITE_TRANSIENT, avoiding Zig's
// pointer alignment check on the ((sqlite3_destructor_type)-1) sentinel.
int sqlite3_bind_text_transient(
    sqlite3_stmt *stmt, int col, const char *text, int len);

#endif
