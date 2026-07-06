# Build Scripts

## macOS Apple Silicon

Run on macOS:

```bash
yarn package:mac:arm64
```

Target:

```text
aarch64-apple-darwin
```

Artifacts:

```text
src-tauri/target/aarch64-apple-darwin/release/bundle
```

## macOS Intel

Run on macOS:

```bash
yarn package:mac:x64
```

Target:

```text
x86_64-apple-darwin
```

Artifacts:

```text
src-tauri/target/x86_64-apple-darwin/release/bundle
```

## Windows amd64

Run on Windows with Rust, Node.js, Yarn, and Microsoft C++ Build Tools installed:

```powershell
yarn package:win:amd64
```

Target:

```text
x86_64-pc-windows-msvc
```

Artifacts:

```text
src-tauri\target\x86_64-pc-windows-msvc\release\bundle
```

Windows packages should be built on Windows or a Windows CI runner. Cross-building Windows Tauri bundles from macOS is not recommended because the WebView2/MSVC bundling toolchain is Windows-native.
