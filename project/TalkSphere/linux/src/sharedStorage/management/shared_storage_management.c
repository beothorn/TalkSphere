#include "shared_storage_management.h"

#include "../../common/result.h"
#include "../../logging.h"

#include <ctype.h>
#include <dlfcn.h>
#include <limits.h>
#include <stdbool.h>
#include <stddef.h>
#include <stdio.h>
#include <string.h>
#include <strings.h>

#define SQLITE_OK 0
#define SQLITE_ROW 100
#define SQLITE_DONE 101
#define SQLITE_OPEN_READWRITE 0x00000002
#define SQLITE_OPEN_CREATE 0x00000004
#define SQLITE_TRANSIENT ((void (*)(void *))-1)

typedef struct sqlite3 sqlite3;
typedef struct sqlite3_stmt sqlite3_stmt;

struct sqlite_runtime {
    void *library_handle;
    int (*open_v2)(const char *, sqlite3 **, int, const char *);
    int (*close)(sqlite3 *);
    int (*exec)(sqlite3 *, const char *, int (*)(void *, int, char **, char **), void *, char **);
    void (*free)(void *);
    int (*prepare_v2)(sqlite3 *, const char *, int, sqlite3_stmt **, const char **);
    int (*step)(sqlite3_stmt *);
    int (*finalize)(sqlite3_stmt *);
    int (*bind_text)(sqlite3_stmt *, int, const char *, int, void (*)(void *));
    int (*bind_int64)(sqlite3_stmt *, int, long long);
    long long (*column_int64)(sqlite3_stmt *, int);
    const unsigned char *(*column_text)(sqlite3_stmt *, int);
    int (*changes)(sqlite3 *);
    int (*stmt_readonly)(sqlite3_stmt *);
    const char *(*errmsg)(sqlite3 *);
};

struct query_callback_bridge_context {
    shared_storage_query_row_callback row_callback;
    void *row_callback_context;
};

static struct sqlite_runtime sqlite_runtime;

static int load_sqlite_symbol(
    const char *symbol_name,
    void **symbol_pointer
) {
    LOG_TRACE("load_sqlite_symbol(): now we load one SQLite function from the runtime library");

    *symbol_pointer = dlsym(
        sqlite_runtime.library_handle,
        symbol_name
    );
    if (*symbol_pointer == NULL) {
        LOG_ERROR("Loading a SQLite runtime symbol failed so shared storage management cannot run");
        return TALKSPHERE_FAILURE;
    }

    return TALKSPHERE_SUCCESS;
}

static int ensure_sqlite_runtime_loaded(void) {
    LOG_TRACE("ensure_sqlite_runtime_loaded(): now we make sure the SQLite runtime is available for file metadata");

    if (sqlite_runtime.library_handle != NULL) {
        return TALKSPHERE_SUCCESS;
    }

    sqlite_runtime.library_handle = dlopen(
        "libsqlite3.so.0",
        RTLD_NOW | RTLD_LOCAL
    );
    if (sqlite_runtime.library_handle == NULL) {
        LOG_ERROR("Opening the SQLite runtime library failed so shared storage metadata cannot be managed");
        return TALKSPHERE_FAILURE;
    }

    if (load_sqlite_symbol(
            "sqlite3_open_v2",
            (void **)&sqlite_runtime.open_v2
        ) != TALKSPHERE_SUCCESS
        || load_sqlite_symbol(
            "sqlite3_close",
            (void **)&sqlite_runtime.close
        ) != TALKSPHERE_SUCCESS
        || load_sqlite_symbol(
            "sqlite3_exec",
            (void **)&sqlite_runtime.exec
        ) != TALKSPHERE_SUCCESS
        || load_sqlite_symbol(
            "sqlite3_free",
            (void **)&sqlite_runtime.free
        ) != TALKSPHERE_SUCCESS
        || load_sqlite_symbol(
            "sqlite3_prepare_v2",
            (void **)&sqlite_runtime.prepare_v2
        ) != TALKSPHERE_SUCCESS
        || load_sqlite_symbol(
            "sqlite3_step",
            (void **)&sqlite_runtime.step
        ) != TALKSPHERE_SUCCESS
        || load_sqlite_symbol(
            "sqlite3_finalize",
            (void **)&sqlite_runtime.finalize
        ) != TALKSPHERE_SUCCESS
        || load_sqlite_symbol(
            "sqlite3_bind_text",
            (void **)&sqlite_runtime.bind_text
        ) != TALKSPHERE_SUCCESS
        || load_sqlite_symbol(
            "sqlite3_bind_int64",
            (void **)&sqlite_runtime.bind_int64
        ) != TALKSPHERE_SUCCESS
        || load_sqlite_symbol(
            "sqlite3_column_int64",
            (void **)&sqlite_runtime.column_int64
        ) != TALKSPHERE_SUCCESS
        || load_sqlite_symbol(
            "sqlite3_column_text",
            (void **)&sqlite_runtime.column_text
        ) != TALKSPHERE_SUCCESS
        || load_sqlite_symbol(
            "sqlite3_changes",
            (void **)&sqlite_runtime.changes
        ) != TALKSPHERE_SUCCESS
        || load_sqlite_symbol(
            "sqlite3_stmt_readonly",
            (void **)&sqlite_runtime.stmt_readonly
        ) != TALKSPHERE_SUCCESS
        || load_sqlite_symbol(
            "sqlite3_errmsg",
            (void **)&sqlite_runtime.errmsg
        ) != TALKSPHERE_SUCCESS
    ) {
        return TALKSPHERE_FAILURE;
    }

    return TALKSPHERE_SUCCESS;
}

static int open_database(
    const char *database_file_path,
    sqlite3 **database
) {
    LOG_TRACE("open_database(): now we open the shared storage SQLite database");

    if (ensure_sqlite_runtime_loaded() != TALKSPHERE_SUCCESS) {
        return TALKSPHERE_FAILURE;
    }

    int open_result = sqlite_runtime.open_v2(
        database_file_path,
        database,
        SQLITE_OPEN_READWRITE | SQLITE_OPEN_CREATE,
        NULL
    );
    if (open_result != SQLITE_OK) {
        LOG_ERROR("Opening the shared storage SQLite database failed");
        return TALKSPHERE_FAILURE;
    }

    return TALKSPHERE_SUCCESS;
}

static int close_database(
    sqlite3 *database
) {
    LOG_TRACE("close_database(): now we close the shared storage SQLite database");

    if (sqlite_runtime.close(database) != SQLITE_OK) {
        LOG_ERROR("Closing the shared storage SQLite database failed");
        return TALKSPHERE_FAILURE;
    }

    return TALKSPHERE_SUCCESS;
}

static int run_database_sql(
    sqlite3 *database,
    const char *sql_statement
) {
    LOG_TRACE("run_database_sql(): now we run a SQLite statement for shared storage metadata");

    char *error_message = NULL;
    int sql_result = sqlite_runtime.exec(
        database,
        sql_statement,
        NULL,
        NULL,
        &error_message
    );

    if (sql_result != SQLITE_OK) {
        LOG_ERROR("Running shared storage SQL failed so metadata may be unchanged");
        if (error_message != NULL) {
            LOG_DEBUG("SQLite reported this shared storage SQL error: %s", error_message);
        }
        sqlite_runtime.free(error_message);
        return TALKSPHERE_FAILURE;
    }

    return TALKSPHERE_SUCCESS;
}

static int prepare_statement(
    sqlite3 *database,
    const char *sql_statement,
    sqlite3_stmt **statement
) {
    LOG_TRACE("prepare_statement(): now we prepare a SQLite statement for shared storage metadata");

    if (sqlite_runtime.prepare_v2(
            database,
            sql_statement,
            -1,
            statement,
            NULL
        ) != SQLITE_OK
    ) {
        LOG_ERROR("Preparing shared storage SQL failed so metadata work cannot continue");
        return TALKSPHERE_FAILURE;
    }

    return TALKSPHERE_SUCCESS;
}

static int finalize_statement(
    sqlite3_stmt *statement
) {
    LOG_TRACE("finalize_statement(): now we release a prepared SQLite statement");

    if (sqlite_runtime.finalize(statement) != SQLITE_OK) {
        LOG_ERROR("Finalizing shared storage SQL failed");
        return TALKSPHERE_FAILURE;
    }

    return TALKSPHERE_SUCCESS;
}

static int bind_required_text(
    sqlite3_stmt *statement,
    int parameter_index,
    const char *text_value
) {
    LOG_TRACE("bind_required_text(): now we bind text into a shared storage SQL statement");

    if (sqlite_runtime.bind_text(
            statement,
            parameter_index,
            text_value,
            -1,
            SQLITE_TRANSIENT
        ) != SQLITE_OK
    ) {
        LOG_ERROR("Binding text into shared storage SQL failed");
        return TALKSPHERE_FAILURE;
    }

    return TALKSPHERE_SUCCESS;
}

static int step_until_done(
    sqlite3_stmt *statement
) {
    LOG_TRACE("step_until_done(): now we execute a SQLite statement that should not return rows");

    int step_result = sqlite_runtime.step(statement);
    if (step_result != SQLITE_DONE) {
        LOG_ERROR("Executing shared storage SQL failed before completion");
        return TALKSPHERE_FAILURE;
    }

    return TALKSPHERE_SUCCESS;
}

static bool sql_query_starts_with_keyword(
    const char *sql_query,
    const char *keyword
) {
    LOG_TRACE("sql_query_starts_with_keyword(): now we check whether a caller SQL statement is read-only by its first keyword");

    while (isspace((unsigned char)*sql_query)) {
        sql_query++;
    }

    size_t keyword_length = strlen(keyword);
    return strncasecmp(
        sql_query,
        keyword,
        keyword_length
    ) == 0;
}

static int validate_read_only_single_statement(
    sqlite3 *database,
    const char *sql_query
) {
    LOG_TRACE("validate_read_only_single_statement(): now we ask SQLite whether the caller query is read-only and single statement");

    sqlite3_stmt *statement = NULL;
    const char *remaining_sql_query = NULL;
    if (sqlite_runtime.prepare_v2(
            database,
            sql_query,
            -1,
            &statement,
            &remaining_sql_query
        ) != SQLITE_OK
    ) {
        LOG_ERROR("Preparing the caller shared storage query failed during read-only validation");
        return TALKSPHERE_FAILURE;
    }

    if (statement == NULL) {
        LOG_WARN("The shared storage query was rejected because it did not contain a SQL statement");
        return TALKSPHERE_FAILURE;
    }

    while (remaining_sql_query != NULL && isspace((unsigned char)*remaining_sql_query)) {
        remaining_sql_query++;
    }

    if (remaining_sql_query != NULL && remaining_sql_query[0] != '\0') {
        LOG_WARN("The shared storage query was rejected because only one SQL statement is allowed");
        finalize_statement(statement);
        return TALKSPHERE_FAILURE;
    }

    if (!sqlite_runtime.stmt_readonly(statement)) {
        LOG_WARN("The shared storage query was rejected because SQLite marked it as a mutating statement");
        finalize_statement(statement);
        return TALKSPHERE_FAILURE;
    }

    return finalize_statement(statement);
}

static int query_callback_bridge(
    void *callback_context,
    int column_count,
    char **column_values,
    char **column_names
) {
    LOG_TRACE("query_callback_bridge(): now we pass one SQLite query row back to the shared storage caller");

    struct query_callback_bridge_context *bridge_context = callback_context;

    return bridge_context->row_callback(
        column_count,
        (const char **)column_names,
        (const char **)column_values,
        bridge_context->row_callback_context
    );
}

int shared_storage_management_prepare(
    const char *database_file_path
) {
    LOG_TRACE("shared_storage_management_prepare(): now we create the shared storage metadata schema if needed");

    sqlite3 *database = NULL;
    if (open_database(
            database_file_path,
            &database
        ) != TALKSPHERE_SUCCESS
    ) {
        return TALKSPHERE_FAILURE;
    }

    int schema_result = run_database_sql(
        database,
        "CREATE TABLE IF NOT EXISTS shared_files ("
        "shared_file_id TEXT NOT NULL,"
        "owner_id TEXT NOT NULL,"
        "stored_file_path TEXT NOT NULL,"
        "file_byte_count INTEGER NOT NULL,"
        "expiration_time_seconds INTEGER NOT NULL,"
        "created_time_seconds INTEGER NOT NULL DEFAULT 0,"
        "PRIMARY KEY (shared_file_id, owner_id)"
        ");"
    );

    int close_result = close_database(database);
    if (schema_result != TALKSPHERE_SUCCESS || close_result != TALKSPHERE_SUCCESS) {
        return TALKSPHERE_FAILURE;
    }

    return TALKSPHERE_SUCCESS;
}

int shared_storage_management_save_entry(
    const char *database_file_path,
    const char *shared_file_id,
    const char *owner_id,
    const char *stored_file_path,
    size_t file_byte_count,
    long long expiration_time_seconds
) {
    LOG_TRACE("shared_storage_management_save_entry(): now we upsert metadata for one shared storage file");
    LOG_DEBUG("Saving shared storage metadata for a file with byte count %zu", file_byte_count);

    sqlite3 *database = NULL;
    sqlite3_stmt *statement = NULL;
    if (open_database(
            database_file_path,
            &database
        ) != TALKSPHERE_SUCCESS
    ) {
        return TALKSPHERE_FAILURE;
    }

    if (prepare_statement(
            database,
            "INSERT INTO shared_files (shared_file_id, owner_id, stored_file_path, file_byte_count, expiration_time_seconds) "
            "VALUES (?1, ?2, ?3, ?4, ?5) "
            "ON CONFLICT(shared_file_id, owner_id) DO UPDATE SET "
            "stored_file_path = excluded.stored_file_path, "
            "file_byte_count = excluded.file_byte_count, "
            "expiration_time_seconds = excluded.expiration_time_seconds",
            &statement
        ) != TALKSPHERE_SUCCESS
        || bind_required_text(
            statement,
            1,
            shared_file_id
        ) != TALKSPHERE_SUCCESS
        || bind_required_text(
            statement,
            2,
            owner_id
        ) != TALKSPHERE_SUCCESS
        || bind_required_text(
            statement,
            3,
            stored_file_path
        ) != TALKSPHERE_SUCCESS
        || sqlite_runtime.bind_int64(
            statement,
            4,
            (long long)file_byte_count
        ) != SQLITE_OK
        || sqlite_runtime.bind_int64(
            statement,
            5,
            expiration_time_seconds
        ) != SQLITE_OK
        || step_until_done(statement) != TALKSPHERE_SUCCESS
    ) {
        if (statement != NULL) {
            finalize_statement(statement);
        }
        close_database(database);
        return TALKSPHERE_FAILURE;
    }

    int finalize_result = finalize_statement(statement);
    int close_result = close_database(database);
    if (finalize_result != TALKSPHERE_SUCCESS || close_result != TALKSPHERE_SUCCESS) {
        return TALKSPHERE_FAILURE;
    }

    return TALKSPHERE_SUCCESS;
}

int shared_storage_management_find_entry(
    const char *database_file_path,
    const char *shared_file_id,
    const char *owner_id,
    struct shared_storage_managed_entry *managed_entry
) {
    LOG_TRACE("shared_storage_management_find_entry(): now we find metadata for a shared storage file");

    sqlite3 *database = NULL;
    sqlite3_stmt *statement = NULL;
    if (open_database(
            database_file_path,
            &database
        ) != TALKSPHERE_SUCCESS
    ) {
        return TALKSPHERE_FAILURE;
    }

    if (prepare_statement(
            database,
            "SELECT stored_file_path, file_byte_count FROM shared_files WHERE shared_file_id = ?1 AND owner_id = ?2",
            &statement
        ) != TALKSPHERE_SUCCESS
        || bind_required_text(
            statement,
            1,
            shared_file_id
        ) != TALKSPHERE_SUCCESS
        || bind_required_text(
            statement,
            2,
            owner_id
        ) != TALKSPHERE_SUCCESS
    ) {
        if (statement != NULL) {
            finalize_statement(statement);
        }
        close_database(database);
        return TALKSPHERE_FAILURE;
    }

    int step_result = sqlite_runtime.step(statement);
    if (step_result != SQLITE_ROW) {
        LOG_WARN("The requested shared storage metadata entry was not found");
        finalize_statement(statement);
        close_database(database);
        return TALKSPHERE_FAILURE;
    }

    const unsigned char *stored_file_path = sqlite_runtime.column_text(
        statement,
        0
    );
    if (stored_file_path == NULL) {
        LOG_ERROR("Shared storage metadata has no file path so the entry cannot be recovered");
        finalize_statement(statement);
        close_database(database);
        return TALKSPHERE_FAILURE;
    }

    if (snprintf(
            managed_entry->stored_file_path,
            sizeof(managed_entry->stored_file_path),
            "%s",
            (const char *)stored_file_path
        ) >= (int)sizeof(managed_entry->stored_file_path)
    ) {
        LOG_ERROR("The stored file path in metadata is too long to recover safely");
        finalize_statement(statement);
        close_database(database);
        return TALKSPHERE_FAILURE;
    }

    managed_entry->file_byte_count = (size_t)sqlite_runtime.column_int64(
        statement,
        1
    );

    int finalize_result = finalize_statement(statement);
    int close_result = close_database(database);
    if (finalize_result != TALKSPHERE_SUCCESS || close_result != TALKSPHERE_SUCCESS) {
        return TALKSPHERE_FAILURE;
    }

    return TALKSPHERE_SUCCESS;
}

int shared_storage_management_delete_entry(
    const char *database_file_path,
    const char *shared_file_id,
    const char *owner_id
) {
    LOG_TRACE("shared_storage_management_delete_entry(): now we delete metadata for one shared storage file");

    sqlite3 *database = NULL;
    sqlite3_stmt *statement = NULL;
    if (open_database(
            database_file_path,
            &database
        ) != TALKSPHERE_SUCCESS
    ) {
        return TALKSPHERE_FAILURE;
    }

    if (prepare_statement(
            database,
            "DELETE FROM shared_files WHERE shared_file_id = ?1 AND owner_id = ?2",
            &statement
        ) != TALKSPHERE_SUCCESS
        || bind_required_text(
            statement,
            1,
            shared_file_id
        ) != TALKSPHERE_SUCCESS
        || bind_required_text(
            statement,
            2,
            owner_id
        ) != TALKSPHERE_SUCCESS
        || step_until_done(statement) != TALKSPHERE_SUCCESS
    ) {
        if (statement != NULL) {
            finalize_statement(statement);
        }
        close_database(database);
        return TALKSPHERE_FAILURE;
    }

    int changed_row_count = sqlite_runtime.changes(database);
    int finalize_result = finalize_statement(statement);
    int close_result = close_database(database);
    if (finalize_result != TALKSPHERE_SUCCESS || close_result != TALKSPHERE_SUCCESS) {
        return TALKSPHERE_FAILURE;
    }

    if (changed_row_count == 0) {
        LOG_WARN("No shared storage metadata row was deleted because the requested entry did not exist");
        return TALKSPHERE_FAILURE;
    }

    return TALKSPHERE_SUCCESS;
}

int shared_storage_management_for_each_expired_entry(
    const char *database_file_path,
    long long current_time_seconds,
    shared_storage_expired_entry_callback expired_entry_callback,
    void *callback_context
) {
    LOG_TRACE("shared_storage_management_for_each_expired_entry(): now we scan expired shared storage metadata entries");

    sqlite3 *database = NULL;
    sqlite3_stmt *statement = NULL;
    if (open_database(
            database_file_path,
            &database
        ) != TALKSPHERE_SUCCESS
    ) {
        return TALKSPHERE_FAILURE;
    }

    if (prepare_statement(
            database,
            "SELECT shared_file_id, owner_id, stored_file_path FROM shared_files WHERE expiration_time_seconds <= ?1",
            &statement
        ) != TALKSPHERE_SUCCESS
        || sqlite_runtime.bind_int64(
            statement,
            1,
            current_time_seconds
        ) != SQLITE_OK
    ) {
        if (statement != NULL) {
            finalize_statement(statement);
        }
        close_database(database);
        return TALKSPHERE_FAILURE;
    }

    while (true) {
        int step_result = sqlite_runtime.step(statement);
        if (step_result == SQLITE_DONE) {
            break;
        }

        if (step_result != SQLITE_ROW) {
            LOG_ERROR("Reading expired shared storage entries failed");
            finalize_statement(statement);
            close_database(database);
            return TALKSPHERE_FAILURE;
        }

        const char *shared_file_id = (const char *)sqlite_runtime.column_text(statement, 0);
        const char *owner_id = (const char *)sqlite_runtime.column_text(statement, 1);
        const char *stored_file_path = (const char *)sqlite_runtime.column_text(statement, 2);

        if (expired_entry_callback(
                shared_file_id,
                owner_id,
                stored_file_path,
                callback_context
            ) != TALKSPHERE_SUCCESS
        ) {
            finalize_statement(statement);
            close_database(database);
            return TALKSPHERE_FAILURE;
        }
    }

    int finalize_result = finalize_statement(statement);
    int close_result = close_database(database);
    if (finalize_result != TALKSPHERE_SUCCESS || close_result != TALKSPHERE_SUCCESS) {
        return TALKSPHERE_FAILURE;
    }

    return TALKSPHERE_SUCCESS;
}

int shared_storage_management_delete_expired_entries(
    const char *database_file_path,
    long long current_time_seconds
) {
    LOG_TRACE("shared_storage_management_delete_expired_entries(): now we delete expired shared storage metadata after files were removed");

    sqlite3 *database = NULL;
    sqlite3_stmt *statement = NULL;
    if (open_database(
            database_file_path,
            &database
        ) != TALKSPHERE_SUCCESS
    ) {
        return TALKSPHERE_FAILURE;
    }

    if (prepare_statement(
            database,
            "DELETE FROM shared_files WHERE expiration_time_seconds <= ?1",
            &statement
        ) != TALKSPHERE_SUCCESS
        || sqlite_runtime.bind_int64(
            statement,
            1,
            current_time_seconds
        ) != SQLITE_OK
        || step_until_done(statement) != TALKSPHERE_SUCCESS
    ) {
        if (statement != NULL) {
            finalize_statement(statement);
        }
        close_database(database);
        return TALKSPHERE_FAILURE;
    }

    int finalize_result = finalize_statement(statement);
    int close_result = close_database(database);
    if (finalize_result != TALKSPHERE_SUCCESS || close_result != TALKSPHERE_SUCCESS) {
        return TALKSPHERE_FAILURE;
    }

    return TALKSPHERE_SUCCESS;
}

int shared_storage_management_query(
    const char *database_file_path,
    const char *sql_query,
    shared_storage_query_row_callback row_callback,
    void *callback_context
) {
    LOG_TRACE("shared_storage_management_query(): now we run a caller read-only SQL query against shared storage metadata");

    if (!sql_query_starts_with_keyword(
            sql_query,
            "SELECT"
        )
        && !sql_query_starts_with_keyword(
            sql_query,
            "WITH"
        )
        && !sql_query_starts_with_keyword(
            sql_query,
            "PRAGMA"
        )
    ) {
        LOG_WARN("The shared storage query was rejected because this API only accepts read-only query statements");
        return TALKSPHERE_FAILURE;
    }

    sqlite3 *database = NULL;
    if (open_database(
            database_file_path,
            &database
        ) != TALKSPHERE_SUCCESS
    ) {
        return TALKSPHERE_FAILURE;
    }

    if (validate_read_only_single_statement(
            database,
            sql_query
        ) != TALKSPHERE_SUCCESS
    ) {
        close_database(database);
        return TALKSPHERE_FAILURE;
    }

    char *error_message = NULL;
    struct query_callback_bridge_context bridge_context = {
        .row_callback = row_callback,
        .row_callback_context = callback_context
    };

    int query_result = sqlite_runtime.exec(
        database,
        sql_query,
        query_callback_bridge,
        &bridge_context,
        &error_message
    );

    if (query_result != SQLITE_OK) {
        LOG_ERROR("Running the shared storage metadata query failed");
        sqlite_runtime.free(error_message);
        close_database(database);
        return TALKSPHERE_FAILURE;
    }

    return close_database(database);
}
