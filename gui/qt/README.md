# ws2tcp-local Qt GUI

This is a Qt Widgets GUI that calls the `ws2tcp-local-ffi` C ABI layer.
The GUI stores user settings with `QSettings` and loads them automatically on
the next startup.

Build the Rust FFI library first:

```bash
cargo build -p ws2tcp-local-ffi
```

Then configure the Qt project with the built library path:

```bash
cmake -S gui/qt -B target/qt-build \
  -DWS2TCP_LOCAL_FFI_LIBRARY="$PWD/target/debug/libws2tcp_local_ffi.so"
cmake --build target/qt-build
cmake --install target/qt-build --prefix target/qt-install
```

On macOS use `libws2tcp_local_ffi.dylib`. On Windows,
`WS2TCP_LOCAL_FFI_LIBRARY` must point to the generated
`ws2tcp_local_ffi.dll`, and `WS2TCP_LOCAL_FFI_IMPLIB` must point to the
generated import library used for linking. The install step also runs Qt's
deployment logic on Windows, including the `qwindows` platform plugin.
