cmake_minimum_required(VERSION 3.20)
project(Rugi VERSION 0.1.0 LANGUAGES C CXX)

# ---------------------------------------------------------------------------
# Rugi — CMake integration.
#
# Builds the Rust C ABI library (crates/rugi-ffi) with Cargo and exposes it
# as the imported target `rugi::rugi` (shared library + include/rugi).
# In another project:
#
#   add_subdirectory(rugi)              # or FetchContent_MakeAvailable(rugi)
#   add_executable(app main.cpp)
#   rugi_link_executable(app)           # links rugi::rugi, sets the rpath and
#                                       # copies the DLL on Windows
#
#   cmake -S . -B build -DCMAKE_BUILD_TYPE=Release
#   cmake --build build
#   ctest --test-dir build --output-on-failure
#
# Sanitizer build of the C/C++ tests and examples (the Rust library itself is
# not instrumented; use the dev profile for its debug assertions):
#
#   cmake -S . -B build-asan -DRUGI_SANITIZE=ON -DRUGI_CARGO_PROFILE=dev
#   cmake --build build-asan && ctest --test-dir build-asan --output-on-failure
# ---------------------------------------------------------------------------

# Examples and tests are built only when Rugi is the top-level project, not
# when another project includes it with add_subdirectory or FetchContent.
if(CMAKE_SOURCE_DIR STREQUAL CMAKE_CURRENT_SOURCE_DIR)
    set(_rugi_top_level ON)
else()
    set(_rugi_top_level OFF)
endif()
option(RUGI_BUILD_EXAMPLES "Build the C++ examples" ${_rugi_top_level})
option(RUGI_BUILD_TESTS "Build the C/C++ interop tests" ${_rugi_top_level})
option(RUGI_SANITIZE "Build the C/C++ tests and examples with AddressSanitizer and UBSan" OFF)
set(RUGI_CARGO_PROFILE "release" CACHE STRING "Cargo profile used for the Rust library (release or dev)")

set(CMAKE_CXX_STANDARD 17)
set(CMAKE_CXX_STANDARD_REQUIRED ON)
set(CMAKE_CXX_EXTENSIONS OFF)

find_program(CARGO_EXECUTABLE cargo HINTS "$ENV{HOME}/.cargo/bin" REQUIRED)

set(RUGI_CARGO_TARGET_DIR "${CMAKE_CURRENT_SOURCE_DIR}/target")
if(RUGI_CARGO_PROFILE STREQUAL "dev")
    set(_rugi_profile_dir "debug")
else()
    set(_rugi_profile_dir "${RUGI_CARGO_PROFILE}")
endif()
set(_rugi_out "${RUGI_CARGO_TARGET_DIR}/${_rugi_profile_dir}")

if(WIN32)
    set(_rugi_lib "${_rugi_out}/rugi_ffi.dll")
    set(_rugi_implib "${_rugi_out}/rugi_ffi.dll.lib")
elseif(APPLE)
    set(_rugi_lib "${_rugi_out}/librugi_ffi.dylib")
else()
    set(_rugi_lib "${_rugi_out}/librugi_ffi.so")
endif()

# Always invoke cargo; it is incremental and tracks Rust sources itself.
add_custom_target(rugi_cargo_build
    COMMAND "${CARGO_EXECUTABLE}" build -p rugi-ffi --profile "${RUGI_CARGO_PROFILE}"
            --target-dir "${RUGI_CARGO_TARGET_DIR}"
    WORKING_DIRECTORY "${CMAKE_CURRENT_SOURCE_DIR}"
    BYPRODUCTS "${_rugi_lib}" ${_rugi_implib}
    COMMENT "Building Rugi Rust library (cargo, profile ${RUGI_CARGO_PROFILE})"
    USES_TERMINAL)

add_library(rugi SHARED IMPORTED GLOBAL)
add_library(rugi::rugi ALIAS rugi)
set_target_properties(rugi PROPERTIES
    IMPORTED_LOCATION "${_rugi_lib}"
    INTERFACE_INCLUDE_DIRECTORIES "${CMAKE_CURRENT_SOURCE_DIR}/include"
    # rugi.hpp needs C++17; rugi.h is plain C and is unaffected.
    INTERFACE_COMPILE_FEATURES cxx_std_17)
if(WIN32)
    set_target_properties(rugi PROPERTIES IMPORTED_IMPLIB "${_rugi_implib}")
endif()
add_dependencies(rugi rugi_cargo_build)

# Links an executable against Rugi: build rpath to the library, a copy of the
# DLL next to the executable on Windows, and strict compiler warnings.
function(rugi_link_executable target)
    target_link_libraries(${target} PRIVATE rugi::rugi)
    add_dependencies(${target} rugi_cargo_build)
    set_target_properties(${target} PROPERTIES BUILD_RPATH "${_rugi_out}")
    if(WIN32)
        add_custom_command(TARGET ${target} POST_BUILD
            COMMAND ${CMAKE_COMMAND} -E copy_if_different "${_rugi_lib}" "$<TARGET_FILE_DIR:${target}>")
    endif()
    if(MSVC)
        target_compile_options(${target} PRIVATE /W4 /utf-8)
    else()
        target_compile_options(${target} PRIVATE -Wall -Wextra -Wpedantic)
    endif()
    if(RUGI_SANITIZE)
        if(MSVC)
            target_compile_options(${target} PRIVATE /fsanitize=address)
        else()
            set(_rugi_san -fsanitize=address,undefined -fno-sanitize-recover=undefined -fno-omit-frame-pointer)
            target_compile_options(${target} PRIVATE ${_rugi_san} -g)
            target_link_options(${target} PRIVATE ${_rugi_san})
        endif()
    endif()
endfunction()

# Runtime options for sanitized test runs. LeakSanitizer is not supported on
# macOS, so leaks are checked there with `leaks --atExit` in the normal build.
set(_rugi_asan_options "halt_on_error=1:abort_on_error=0:detect_stack_use_after_return=1:strict_string_checks=1:max_free_fill_size=4096:free_fill_byte=190")
if(NOT APPLE)
    string(APPEND _rugi_asan_options ":detect_leaks=1")
endif()
function(rugi_add_test name target)
    add_test(NAME ${name} COMMAND ${target} ${ARGN})
    if(RUGI_SANITIZE)
        set_tests_properties(${name} PROPERTIES ENVIRONMENT
            "ASAN_OPTIONS=${_rugi_asan_options};UBSAN_OPTIONS=print_stacktrace=1:halt_on_error=1")
    endif()
endfunction()

if(RUGI_BUILD_EXAMPLES)
    add_executable(rugi_cpp_demo examples/cpp-demo/main.cpp)
    rugi_link_executable(rugi_cpp_demo)
    add_executable(rugi_cpp_glass examples/cpp-glass/main.cpp)
    rugi_link_executable(rugi_cpp_glass)
endif()

if(RUGI_BUILD_TESTS)
    enable_testing()
    add_executable(rugi_cpp_interop_test tests/cpp/interop_test.cpp)
    rugi_link_executable(rugi_cpp_interop_test)
    rugi_add_test(rugi_cpp_interop rugi_cpp_interop_test)

    add_executable(rugi_c_smoke tests/cpp/c_smoke.c)
    set_target_properties(rugi_c_smoke PROPERTIES C_STANDARD 11 C_STANDARD_REQUIRED ON)
    rugi_link_executable(rugi_c_smoke)
    rugi_add_test(rugi_c_smoke rugi_c_smoke)

    # Every exported function with hostile arguments, handles and callbacks.
    # The tests use POSIX threads, so they are not built for Windows.
    if(NOT WIN32)
        add_executable(rugi_c_hostile tests/cpp/hostile.c)
        set_target_properties(rugi_c_hostile PROPERTIES C_STANDARD 11 C_STANDARD_REQUIRED ON)
        rugi_link_executable(rugi_c_hostile)
        find_package(Threads REQUIRED)
        target_link_libraries(rugi_c_hostile PRIVATE Threads::Threads)
        if(NOT MSVC)
            target_link_libraries(rugi_c_hostile PRIVATE m)
        endif()
        rugi_add_test(rugi_c_hostile rugi_c_hostile)

        add_executable(rugi_cpp_hostile tests/cpp/hostile.cpp)
        rugi_link_executable(rugi_cpp_hostile)
        target_link_libraries(rugi_cpp_hostile PRIVATE Threads::Threads)
        rugi_add_test(rugi_cpp_hostile rugi_cpp_hostile)
    endif()

    # Leak check of the hostile tests (macOS `leaks`; ASan covers Linux).
    find_program(RUGI_LEAKS_EXECUTABLE leaks)
    if(APPLE AND RUGI_LEAKS_EXECUTABLE AND NOT RUGI_SANITIZE)
        add_test(NAME rugi_c_hostile_leaks
                 COMMAND "${RUGI_LEAKS_EXECUTABLE}" -quiet --atExit -- $<TARGET_FILE:rugi_c_hostile>)
        add_test(NAME rugi_cpp_hostile_leaks
                 COMMAND "${RUGI_LEAKS_EXECUTABLE}" -quiet --atExit -- $<TARGET_FILE:rugi_cpp_hostile>)
        set_tests_properties(rugi_c_hostile_leaks rugi_cpp_hostile_leaks PROPERTIES ENVIRONMENT "MallocStackLogging=1")
    endif()

    # The public headers compile warning-free in every supported language mode.
    if(NOT MSVC)
        set(_rugi_strict -Wall -Wextra -Wpedantic -Werror -fsyntax-only "-I${CMAKE_CURRENT_SOURCE_DIR}/include")
        foreach(std 99 11)
            add_test(NAME rugi_header_c${std}
                     COMMAND "${CMAKE_C_COMPILER}" -std=c${std} ${_rugi_strict} -Wstrict-prototypes
                             -x c "${CMAKE_CURRENT_SOURCE_DIR}/tests/cpp/header_check.c")
        endforeach()
        foreach(std 17 20)
            add_test(NAME rugi_header_cxx${std}
                     COMMAND "${CMAKE_CXX_COMPILER}" -std=c++${std} ${_rugi_strict}
                             -x c++ "${CMAKE_CURRENT_SOURCE_DIR}/tests/cpp/header_check.cpp")
        endforeach()
    endif()
endif()
