cmake_minimum_required(VERSION 3.21)

project(ws2tcp-local-qt VERSION 0.1.5 LANGUAGES CXX)

include(GNUInstallDirs)

set(CMAKE_CXX_STANDARD 17)
set(CMAKE_CXX_STANDARD_REQUIRED ON)
set(CMAKE_AUTOMOC ON)

find_package(Qt6 REQUIRED COMPONENTS Widgets)

set(WS2TCP_LOCAL_FFI_INCLUDE_DIR
    "${CMAKE_CURRENT_LIST_DIR}/../../crates/ws2tcp-local-ffi/include"
    CACHE PATH "Path to ws2tcp-local FFI headers")
set(WS2TCP_LOCAL_FFI_LIBRARY
    ""
    CACHE FILEPATH "Path to ws2tcp-local FFI runtime library (.so, .dylib, or .dll)")
set(WS2TCP_LOCAL_FFI_IMPLIB
    ""
    CACHE FILEPATH "Path to ws2tcp-local FFI import library on Windows")

if(NOT WS2TCP_LOCAL_FFI_LIBRARY)
    message(FATAL_ERROR "Set WS2TCP_LOCAL_FFI_LIBRARY to the built ws2tcp_local_ffi runtime library")
endif()

if(WIN32 AND NOT WS2TCP_LOCAL_FFI_IMPLIB)
    message(FATAL_ERROR "Set WS2TCP_LOCAL_FFI_IMPLIB to the ws2tcp_local_ffi import library on Windows")
endif()

add_library(ws2tcp_local_ffi SHARED IMPORTED)
set_target_properties(ws2tcp_local_ffi PROPERTIES
    IMPORTED_LOCATION "${WS2TCP_LOCAL_FFI_LIBRARY}"
    IMPORTED_NO_SONAME TRUE
    INTERFACE_INCLUDE_DIRECTORIES "${WS2TCP_LOCAL_FFI_INCLUDE_DIR}")

if(WS2TCP_LOCAL_FFI_IMPLIB)
    set_target_properties(ws2tcp_local_ffi PROPERTIES
        IMPORTED_IMPLIB "${WS2TCP_LOCAL_FFI_IMPLIB}")
endif()

add_executable(ws2tcp-local-qt
    src/main.cpp
    src/MainWindow.cpp
    src/MainWindow.h)

if(WIN32)
    set_target_properties(ws2tcp-local-qt PROPERTIES
        WIN32_EXECUTABLE "$<NOT:$<CONFIG:Debug>>")
endif()

target_link_libraries(ws2tcp-local-qt PRIVATE Qt6::Widgets ws2tcp_local_ffi)

if(WIN32)
    qt_generate_deploy_app_script(
        TARGET ws2tcp-local-qt
        OUTPUT_SCRIPT WS2TCP_LOCAL_QT_DEPLOY_SCRIPT
        INCLUDE_PLUGINS qwindows
        NO_TRANSLATIONS
        NO_UNSUPPORTED_PLATFORM_ERROR)
endif()

install(TARGETS ws2tcp-local-qt
    RUNTIME_DEPENDENCY_SET ws2tcp-local-qt-runtime-deps
    BUNDLE DESTINATION .
    RUNTIME DESTINATION "${CMAKE_INSTALL_BINDIR}")

install(IMPORTED_RUNTIME_ARTIFACTS ws2tcp_local_ffi
    RUNTIME_DEPENDENCY_SET ws2tcp-local-qt-runtime-deps
    LIBRARY DESTINATION "${CMAKE_INSTALL_LIBDIR}"
    RUNTIME DESTINATION "${CMAKE_INSTALL_BINDIR}")

install(RUNTIME_DEPENDENCY_SET ws2tcp-local-qt-runtime-deps
    DIRECTORIES
        "$<TARGET_FILE_DIR:Qt6::Core>"
        "$<TARGET_FILE_DIR:Qt6::Gui>"
        "$<TARGET_FILE_DIR:Qt6::Widgets>"
        "$<TARGET_FILE_DIR:ws2tcp_local_ffi>"
    PRE_EXCLUDE_REGEXES
        "api-ms-"
        "ext-ms-"
    POST_EXCLUDE_REGEXES
        ".*[\\\\/]system32[\\\\/].*"
    LIBRARY DESTINATION "${CMAKE_INSTALL_LIBDIR}"
    RUNTIME DESTINATION "${CMAKE_INSTALL_BINDIR}")

if(WIN32)
    install(CODE "
if(IS_ABSOLUTE \"\${CMAKE_INSTALL_PREFIX}\")
    set(QT_DEPLOY_PREFIX \"\$ENV{DESTDIR}\${CMAKE_INSTALL_PREFIX}\")
else()
    get_filename_component(QT_DEPLOY_PREFIX \"\$ENV{DESTDIR}\${CMAKE_INSTALL_PREFIX}\" ABSOLUTE)
endif()
include(\"${WS2TCP_LOCAL_QT_DEPLOY_SCRIPT}\")
")
endif()
