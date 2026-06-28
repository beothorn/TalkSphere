#ifndef TALKSPHERE_TEST_SUPPORT_H
#define TALKSPHERE_TEST_SUPPORT_H

#include <stddef.h>
#include <stdio.h>

#define TEST_SUCCESS 0
#define TEST_FAILURE 1

typedef int (*test_function)(void);

struct test_case {
    const char *name;
    test_function function;
};

static int run_test_case(
    const struct test_case *test_case
) {
    printf(
        "  >>> running %s\n",
        test_case->name
    );

    int test_result = test_case->function();
    if (test_result != TEST_SUCCESS) {
        printf(
            "  !!! failed %s (result code %d)\n",
            test_case->name,
            test_result
        );
        printf(
            "      output above belongs to %s\n",
            test_case->name
        );
        return test_result;
    }

    printf(
        "  <<< passed %s\n",
        test_case->name
    );
    return TEST_SUCCESS;
}

#define TEST_CASE(test_case_function) {#test_case_function, test_case_function}

#define TEST_ASSERT(test_condition, failure_message) \
    do { \
        if (!(test_condition)) { \
            printf( \
                "      failure at %s:%d: %s\n", \
                __FILE__, \
                __LINE__, \
                failure_message \
            ); \
            printf( \
                "      failed condition: %s\n", \
                #test_condition \
            ); \
            return TEST_FAILURE; \
        } \
    } while (0)

#define TEST_ASSERT_RESULT(test_result_expression, expected_result, failure_message) \
    do { \
        int actual_test_result = (test_result_expression); \
        if (actual_test_result != (expected_result)) { \
            printf( \
                "      failure at %s:%d: %s\n", \
                __FILE__, \
                __LINE__, \
                failure_message \
            ); \
            printf( \
                "      expected result: %d\n", \
                (expected_result) \
            ); \
            printf( \
                "      actual result: %d\n", \
                actual_test_result \
            ); \
            printf( \
                "      expression: %s\n", \
                #test_result_expression \
            ); \
            return TEST_FAILURE; \
        } \
    } while (0)

static int run_test_cases(
    const struct test_case test_cases[],
    size_t test_case_count
) {
    for (size_t test_case_index = 0; test_case_index < test_case_count; test_case_index++) {
        int test_result = run_test_case(&test_cases[test_case_index]);
        if (test_result != TEST_SUCCESS) {
            return test_result;
        }
    }

    return TEST_SUCCESS;
}

#endif
