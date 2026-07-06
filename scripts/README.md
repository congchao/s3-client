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

Run on Windows PowerShell 5.1 or PowerShell 7 with Rust, Node.js, Yarn, CMake, and Microsoft C++ Build Tools installed:

```powershell
yarn package:win:amd64
```

Target:

```text
x86_64-pc-windows-msvc
```

Artifacts:

```text
C:\s3-client-target\x86_64-pc-windows-msvc\release\bundle
```

Windows packages should be built on Windows or a Windows CI runner. Cross-building Windows Tauri bundles from macOS is not recommended because the WebView2/MSVC bundling toolchain is Windows-native.

If the build fails with `Missing dependency: cmake`, install CMake and reopen the terminal:

```powershell
winget install Kitware.CMake
```

The Rust dependency `aws-lc-sys` compiles C code on Windows. Make sure Visual Studio Build Tools includes:

- Desktop development with C++
- MSVC v143 or newer
- Windows 10/11 SDK
- C++ CMake tools for Windows

If the build fails while compiling `aws-lc-sys` or `compiler_features_tests\c11.c`, verify the MSVC compiler is available:

```powershell
cl.exe /Bv
```

If `cl.exe` is not found, install Visual Studio Build Tools or run the package command from `x64 Native Tools Command Prompt for VS`.

If the build fails with `C atomics require C11 or later`, make sure you are using the latest Windows build script. It sets:

```powershell
AWS_LC_SYS_C_STD=11
CFLAGS=/std:c11
```

After changing compiler flags, clean the previous Rust target cache before retrying:

```powershell
Remove-Item -Recurse -Force C:\s3-client-target
yarn.cmd package:win:amd64
```

If MSBuild reports `DirectoryNotFoundException` for a deep `CMakeScratch\...\*.tlog` path, it is usually a Windows path length issue. The Windows package script sets `CARGO_TARGET_DIR=C:\s3-client-target` by default to keep build paths short. You can also extract the project to a shorter directory such as `C:\src\s3-client`.
