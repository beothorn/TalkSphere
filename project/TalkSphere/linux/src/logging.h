#ifndef TALKSPHERE_LOGGING_H
#define TALKSPHERE_LOGGING_H

#include <stdarg.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

#define TALKSPHERE_LOG_LEVEL_ENVIRONMENT_VARIABLE "TALKSPHERE_LOG_LEVEL"

enum talksphere_log_level {
    TRACE,
    DEBUG,
    INFO,
    WARN,
    ERROR,
    FATAL,
    NONE
};

/*
 * This shared logger keeps log lines consistent so they are easy to filter.
 */
static inline const char *talksphere_log_level_name(
    enum talksphere_log_level log_level
) {
    switch (log_level) {
        case NONE:
            return "none";
        case TRACE:
            return "trace";
        case DEBUG:
            return "debug";
        case INFO:
            return "info";
        case WARN:
            return "warn";
        case ERROR:
            return "error";
        case FATAL:
            return "fatal";
    }

    return "unknown";
}

static inline int talksphere_strings_are_equal(
    const char *first_text,
    const char *second_text
) {
    return strcmp(
        first_text,
        second_text
    ) == 0;
}

static inline enum talksphere_log_level talksphere_configured_log_level(void) {
    const char *configured_log_level = getenv(TALKSPHERE_LOG_LEVEL_ENVIRONMENT_VARIABLE);

    if (configured_log_level == NULL) {
        return NONE;
    }

    if (talksphere_strings_are_equal(
            configured_log_level,
            "trace"
        )
    ) {
        return TRACE;
    }

    if (talksphere_strings_are_equal(
            configured_log_level,
            "debug"
        )
    ) {
        return DEBUG;
    }

    if (talksphere_strings_are_equal(
            configured_log_level,
            "info"
        )
    ) {
        return INFO;
    }

    if (talksphere_strings_are_equal(
            configured_log_level,
            "warn"
        )
    ) {
        return WARN;
    }

    if (talksphere_strings_are_equal(
            configured_log_level,
            "error"
        )
    ) {
        return ERROR;
    }

    if (talksphere_strings_are_equal(
            configured_log_level,
            "fatal"
        )
    ) {
        return FATAL;
    }

    return INFO;
}

static inline int talksphere_should_log(
    enum talksphere_log_level log_level
) {
    enum talksphere_log_level configured_log_level = talksphere_configured_log_level();

    return log_level >= configured_log_level;
}

static inline void talksphere_log(
    enum talksphere_log_level log_level,
    const char *component_name,
    const char *function_name,
    const char *format,
    ...
) {
    if (!talksphere_should_log(log_level)) {
        return;
    }

    fprintf(
        stderr,
        "[%s] [%s] [%s] ",
        talksphere_log_level_name(log_level),
        component_name,
        function_name
    );

    va_list log_arguments;
    va_start(
        log_arguments,
        format
    );
    vfprintf(
        stderr,
        format,
        log_arguments
    );
    va_end(log_arguments);

    fputc(
        '\n',
        stderr
    );
}

#define LOG_TRACE(...) \
    talksphere_log(TRACE, __FILE__, __func__, __VA_ARGS__)

#define LOG_DEBUG(...) \
    talksphere_log(DEBUG, __FILE__, __func__, __VA_ARGS__)

#define LOG_INFO(...) \
    talksphere_log(INFO, __FILE__, __func__, __VA_ARGS__)

#define LOG_WARN(...) \
    talksphere_log(WARN, __FILE__, __func__, __VA_ARGS__)

#define LOG_ERROR(...) \
    talksphere_log(ERROR, __FILE__, __func__, __VA_ARGS__)

#define LOG_FATAL(...) \
    talksphere_log(FATAL, __FILE__, __func__, __VA_ARGS__)

#endif
