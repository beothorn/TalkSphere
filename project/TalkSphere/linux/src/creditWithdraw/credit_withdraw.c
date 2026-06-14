#include "credit_withdraw.h"

#include "../common/result.h"
#include "../logging.h"

#include <dlfcn.h>
#include <errno.h>
#include <limits.h>
#include <stdbool.h>
#include <stdio.h>
#include <string.h>
#include <sys/stat.h>
#include <sys/types.h>

#ifndef PATH_MAX
#define PATH_MAX 4096
#endif

#define CREDIT_WITHDRAW_DIRECTORY_NAME "creditWithdraw"
#define CREDIT_WITHDRAW_DATABASE_FILE_NAME "credit_withdraw.sqlite"
#define CREDIT_WITHDRAW_DIRECTORY_MODE 0700
#define SQLITE_OK 0
#define SQLITE_ROW 100
#define SQLITE_DONE 101
#define SQLITE_OPEN_READWRITE 0x00000002
#define SQLITE_OPEN_CREATE 0x00000004
#define SQLITE_BIND_TEXT_LENGTH -1
#define SQLITE_PARAMETER_CODE 1
#define SQLITE_PARAMETER_OWNER_IDENTIFIER 2
#define SQLITE_PARAMETER_CREDIT_COUNT 3
#define SQLITE_COLUMN_OWNER_IDENTIFIER 0
#define SQLITE_COLUMN_CREDIT_COUNT 1
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
};

static struct sqlite_runtime sqlite_runtime;

static int load_sqlite_symbol(
    const char *symbol_name,
    void **symbol_pointer
) {
    LOG_TRACE("load_sqlite_symbol(): now we load one SQLite function for credit withdraw storage");

    *symbol_pointer = dlsym(
        sqlite_runtime.library_handle,
        symbol_name
    );
    if (*symbol_pointer == NULL) {
        LOG_ERROR("Loading a SQLite runtime symbol failed so credit withdraw storage cannot run");
        return TALKSPHERE_FAILURE;
    }

    return TALKSPHERE_SUCCESS;
}

static int ensure_sqlite_runtime_loaded(void) {
    LOG_TRACE("ensure_sqlite_runtime_loaded(): now we make sure SQLite is available for credit withdraw storage");

    if (sqlite_runtime.library_handle != NULL) {
        return TALKSPHERE_SUCCESS;
    }

    sqlite_runtime.library_handle = dlopen(
        "libsqlite3.so.0",
        RTLD_NOW | RTLD_LOCAL
    );
    if (sqlite_runtime.library_handle == NULL) {
        LOG_ERROR("Opening the SQLite runtime library failed so credit withdraw codes cannot be managed");
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
    ) {
        return TALKSPHERE_FAILURE;
    }

    return TALKSPHERE_SUCCESS;
}

static int build_credit_withdraw_directory_path(
    const char *app_storage_directory_path,
    char *credit_withdraw_directory_path,
    size_t credit_withdraw_directory_path_size
) {
    LOG_TRACE("build_credit_withdraw_directory_path(): now we build the home subfolder for credit withdraw codes");

    if (snprintf(
            credit_withdraw_directory_path,
            credit_withdraw_directory_path_size,
            "%s/%s",
            app_storage_directory_path,
            CREDIT_WITHDRAW_DIRECTORY_NAME
        ) >= (int)credit_withdraw_directory_path_size
    ) {
        LOG_ERROR("The credit withdraw directory path is too long so credit code storage cannot continue");
        return TALKSPHERE_FAILURE;
    }

    return TALKSPHERE_SUCCESS;
}

static int build_database_file_path(
    const char *app_storage_directory_path,
    char *database_file_path,
    size_t database_file_path_size
) {
    LOG_TRACE("build_database_file_path(): now we build the SQLite database path for credit withdraw codes");

    if (snprintf(
            database_file_path,
            database_file_path_size,
            "%s/%s/%s",
            app_storage_directory_path,
            CREDIT_WITHDRAW_DIRECTORY_NAME,
            CREDIT_WITHDRAW_DATABASE_FILE_NAME
        ) >= (int)database_file_path_size
    ) {
        LOG_ERROR("The credit withdraw database path is too long so credit code storage cannot continue");
        return TALKSPHERE_FAILURE;
    }

    return TALKSPHERE_SUCCESS;
}

static int directory_exists(
    const char *directory_path,
    bool *directory_found
) {
    LOG_TRACE("directory_exists(): now we check whether the credit withdraw directory already exists");

    struct stat directory_status;
    if (stat(
            directory_path,
            &directory_status
        ) != 0
    ) {
        if (errno == ENOENT) {
            *directory_found = false;
            return TALKSPHERE_SUCCESS;
        }

        LOG_ERROR("Checking the credit withdraw directory failed so credit code storage cannot continue");
        return TALKSPHERE_FAILURE;
    }

    *directory_found = S_ISDIR(directory_status.st_mode);
    if (!*directory_found) {
        LOG_ERROR("The credit withdraw path exists but is not a directory");
        return TALKSPHERE_FAILURE;
    }

    return TALKSPHERE_SUCCESS;
}

static int create_credit_withdraw_directory_if_missing(
    const char *app_storage_directory_path
) {
    LOG_TRACE("create_credit_withdraw_directory_if_missing(): now we ensure the home credit withdraw folder exists");

    char credit_withdraw_directory_path[PATH_MAX];
    if (build_credit_withdraw_directory_path(
            app_storage_directory_path,
            credit_withdraw_directory_path,
            sizeof(credit_withdraw_directory_path)
        ) != TALKSPHERE_SUCCESS
    ) {
        return TALKSPHERE_FAILURE;
    }

    bool credit_withdraw_directory_found = false;
    if (directory_exists(
            credit_withdraw_directory_path,
            &credit_withdraw_directory_found
        ) != TALKSPHERE_SUCCESS
    ) {
        return TALKSPHERE_FAILURE;
    }

    if (credit_withdraw_directory_found) {
        return TALKSPHERE_SUCCESS;
    }

    LOG_INFO("Creating credit withdraw directory because it was not present");
    if (mkdir(
            credit_withdraw_directory_path,
            CREDIT_WITHDRAW_DIRECTORY_MODE
        ) != 0
    ) {
        LOG_ERROR("Creating the credit withdraw directory failed so credit code storage cannot continue");
        return TALKSPHERE_FAILURE;
    }

    return TALKSPHERE_SUCCESS;
}

static int open_database(
    const char *database_file_path,
    sqlite3 **database
) {
    LOG_TRACE("open_database(): now we open the credit withdraw SQLite database");

    if (ensure_sqlite_runtime_loaded() != TALKSPHERE_SUCCESS) {
        return TALKSPHERE_FAILURE;
    }

    if (sqlite_runtime.open_v2(
            database_file_path,
            database,
            SQLITE_OPEN_READWRITE | SQLITE_OPEN_CREATE,
            NULL
        ) != SQLITE_OK
    ) {
        LOG_ERROR("Opening the credit withdraw SQLite database failed");
        return TALKSPHERE_FAILURE;
    }

    return TALKSPHERE_SUCCESS;
}

static int close_database(
    sqlite3 *database
) {
    LOG_TRACE("close_database(): now we close the credit withdraw SQLite database");

    if (sqlite_runtime.close(database) != SQLITE_OK) {
        LOG_ERROR("Closing the credit withdraw SQLite database failed");
        return TALKSPHERE_FAILURE;
    }

    return TALKSPHERE_SUCCESS;
}

static int run_database_sql(
    sqlite3 *database,
    const char *sql_statement
) {
    LOG_TRACE("run_database_sql(): now we run schema SQL for credit withdraw codes");

    char *error_message = NULL;
    int sql_result = sqlite_runtime.exec(
        database,
        sql_statement,
        NULL,
        NULL,
        &error_message
    );
    if (sql_result != SQLITE_OK) {
        LOG_ERROR("Running credit withdraw SQL failed so code storage may be unchanged");
        if (error_message != NULL) {
            LOG_DEBUG("SQLite reported this credit withdraw SQL error: %s", error_message);
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
    LOG_TRACE("prepare_statement(): now we prepare a credit withdraw SQL statement");

    if (sqlite_runtime.prepare_v2(
            database,
            sql_statement,
            SQLITE_BIND_TEXT_LENGTH,
            statement,
            NULL
        ) != SQLITE_OK
    ) {
        LOG_ERROR("Preparing credit withdraw SQL failed so code storage cannot continue");
        return TALKSPHERE_FAILURE;
    }

    return TALKSPHERE_SUCCESS;
}

static int finalize_statement(
    sqlite3_stmt *statement
) {
    LOG_TRACE("finalize_statement(): now we release a credit withdraw SQL statement");

    if (sqlite_runtime.finalize(statement) != SQLITE_OK) {
        LOG_ERROR("Finalizing credit withdraw SQL failed");
        return TALKSPHERE_FAILURE;
    }

    return TALKSPHERE_SUCCESS;
}

static int bind_required_text(
    sqlite3_stmt *statement,
    int parameter_index,
    const char *text_value
) {
    LOG_TRACE("bind_required_text(): now we bind text into a credit withdraw SQL statement");

    if (text_value == NULL || text_value[0] == '\0') {
        LOG_WARN("Credit withdraw text is unwanted because required text cannot be empty");
        return TALKSPHERE_FAILURE;
    }

    if (sqlite_runtime.bind_text(
            statement,
            parameter_index,
            text_value,
            SQLITE_BIND_TEXT_LENGTH,
            SQLITE_TRANSIENT
        ) != SQLITE_OK
    ) {
        LOG_ERROR("Binding text into credit withdraw SQL failed");
        return TALKSPHERE_FAILURE;
    }

    return TALKSPHERE_SUCCESS;
}

static int step_until_done(
    sqlite3_stmt *statement
) {
    LOG_TRACE("step_until_done(): now we execute a credit withdraw SQL statement that should not return rows");

    if (sqlite_runtime.step(statement) != SQLITE_DONE) {
        LOG_ERROR("Executing credit withdraw SQL failed before completion");
        return TALKSPHERE_FAILURE;
    }

    return TALKSPHERE_SUCCESS;
}

static int prepare_storage(
    const char *app_storage_directory_path,
    char *database_file_path,
    size_t database_file_path_size
) {
    LOG_TRACE("prepare_storage(): now we prepare the folder and schema for credit withdraw codes");

    if (create_credit_withdraw_directory_if_missing(app_storage_directory_path) != TALKSPHERE_SUCCESS
        || build_database_file_path(
            app_storage_directory_path,
            database_file_path,
            database_file_path_size
        ) != TALKSPHERE_SUCCESS
    ) {
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

    int schema_result = run_database_sql(
        database,
        "CREATE TABLE IF NOT EXISTS credit_withdraw_codes ("
        "code TEXT NOT NULL PRIMARY KEY,"
        "owner_identifier TEXT NOT NULL,"
        "credit_count INTEGER NOT NULL CHECK (credit_count > 0)"
        ");"
    );
    int close_result = close_database(database);

    if (schema_result != TALKSPHERE_SUCCESS || close_result != TALKSPHERE_SUCCESS) {
        return TALKSPHERE_FAILURE;
    }

    return TALKSPHERE_SUCCESS;
}

int credit_withdraw_add_code(
    const char *app_storage_directory_path,
    const char *owner_identifier_text,
    int credit_count,
    const char *credit_code_text
) {
    LOG_TRACE("credit_withdraw_add_code(): now we store or update a code that can later withdraw credits for this id");
    LOG_DEBUG(
        "Adding a credit withdraw code with %d credits",
        credit_count
    );

    if (credit_count <= 0) {
        LOG_WARN("Credit withdraw code is unwanted because the credit count must be positive");
        return TALKSPHERE_FAILURE;
    }

    char database_file_path[PATH_MAX];
    if (prepare_storage(
            app_storage_directory_path,
            database_file_path,
            sizeof(database_file_path)
        ) != TALKSPHERE_SUCCESS
    ) {
        return TALKSPHERE_FAILURE;
    }

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
            "INSERT INTO credit_withdraw_codes (code, owner_identifier, credit_count) "
            "VALUES (?1, ?2, ?3) "
            "ON CONFLICT(code) DO UPDATE SET "
            "owner_identifier = excluded.owner_identifier, "
            "credit_count = excluded.credit_count",
            &statement
        ) != TALKSPHERE_SUCCESS
        || bind_required_text(
            statement,
            SQLITE_PARAMETER_CODE,
            credit_code_text
        ) != TALKSPHERE_SUCCESS
        || bind_required_text(
            statement,
            SQLITE_PARAMETER_OWNER_IDENTIFIER,
            owner_identifier_text
        ) != TALKSPHERE_SUCCESS
        || sqlite_runtime.bind_int64(
            statement,
            SQLITE_PARAMETER_CREDIT_COUNT,
            credit_count
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

int credit_withdraw_remove_code(
    const char *app_storage_directory_path,
    const char *credit_code_text
) {
    LOG_TRACE("credit_withdraw_remove_code(): now we remove one credit withdraw code from the home database");

    char database_file_path[PATH_MAX];
    if (prepare_storage(
            app_storage_directory_path,
            database_file_path,
            sizeof(database_file_path)
        ) != TALKSPHERE_SUCCESS
    ) {
        return TALKSPHERE_FAILURE;
    }

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
            "DELETE FROM credit_withdraw_codes WHERE code = ?1",
            &statement
        ) != TALKSPHERE_SUCCESS
        || bind_required_text(
            statement,
            SQLITE_PARAMETER_CODE,
            credit_code_text
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
        LOG_WARN("No credit withdraw code was removed because the requested code does not exist");
        return TALKSPHERE_FAILURE;
    }

    return TALKSPHERE_SUCCESS;
}

int credit_withdraw_find_code(
    const char *app_storage_directory_path,
    const char *credit_code_text,
    struct credit_withdraw_entry *credit_withdraw_entry
) {
    LOG_TRACE("credit_withdraw_find_code(): now we find who owns the credit behind a withdraw code");

    char database_file_path[PATH_MAX];
    if (prepare_storage(
            app_storage_directory_path,
            database_file_path,
            sizeof(database_file_path)
        ) != TALKSPHERE_SUCCESS
    ) {
        return TALKSPHERE_FAILURE;
    }

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
            "SELECT owner_identifier, credit_count FROM credit_withdraw_codes WHERE code = ?1",
            &statement
        ) != TALKSPHERE_SUCCESS
        || bind_required_text(
            statement,
            SQLITE_PARAMETER_CODE,
            credit_code_text
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
        LOG_WARN("The requested credit withdraw code was not found");
        finalize_statement(statement);
        close_database(database);
        return TALKSPHERE_FAILURE;
    }

    const unsigned char *owner_identifier_text = sqlite_runtime.column_text(
        statement,
        SQLITE_COLUMN_OWNER_IDENTIFIER
    );
    if (owner_identifier_text == NULL) {
        LOG_ERROR("Credit withdraw row has no owner id so it cannot be used");
        finalize_statement(statement);
        close_database(database);
        return TALKSPHERE_FAILURE;
    }

    if (snprintf(
            credit_withdraw_entry->owner_identifier_text,
            sizeof(credit_withdraw_entry->owner_identifier_text),
            "%s",
            (const char *)owner_identifier_text
        ) >= (int)sizeof(credit_withdraw_entry->owner_identifier_text)
    ) {
        LOG_ERROR("Credit withdraw owner id is too long to copy safely");
        finalize_statement(statement);
        close_database(database);
        return TALKSPHERE_FAILURE;
    }

    credit_withdraw_entry->credit_count = (int)sqlite_runtime.column_int64(
        statement,
        SQLITE_COLUMN_CREDIT_COUNT
    );

    int finalize_result = finalize_statement(statement);
    int close_result = close_database(database);
    if (finalize_result != TALKSPHERE_SUCCESS || close_result != TALKSPHERE_SUCCESS) {
        return TALKSPHERE_FAILURE;
    }

    return TALKSPHERE_SUCCESS;
}
