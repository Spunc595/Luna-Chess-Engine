# Building the Android ARM64 executable

How to build Luna's Android ARM64 release binary from source, on a
Windows PC, without Android Studio or Gradle.

## Requirements

- [rustup](https://rustup.rs/), with the `aarch64-linux-android` target
  installed (`rustup target add aarch64-linux-android`). Used for the
  v4.1.1 build: rustc 1.92.0. `Cargo.toml`'s `rust-version` field
  requires at least 1.83.
- Android NDK **r27d** (`Pkg.Revision 27.3.13750724` in the NDK's own
  `source.properties`), targeting **API level 21**. Download it from
  Google's NDK downloads page and extract it anywhere; this guide
  refers to that extracted folder as `$NDK`.

## Build from a clean archive, not your working copy

Build from a clean `git archive` of the tag being released, not from
a working directory that may contain local edits or leftover build
artifacts:

```powershell
git archive v4.1.1 -o luna-source.tar
mkdir android_build
cd android_build
tar -xf ../luna-source.tar
```

(Replace `v4.1.1` with whichever tag you are building.)

## Recipe

From inside that clean, extracted source tree, in PowerShell:

```powershell
$NDK = "C:\path\to\android-ndk-r27d"
$TOOLCHAIN = "$NDK\toolchains\llvm\prebuilt\windows-x86_64\bin"

$env:RUSTFLAGS = ""
$env:CARGO_TARGET_AARCH64_LINUX_ANDROID_LINKER = "$TOOLCHAIN\aarch64-linux-android21-clang.cmd"
$env:CC_aarch64_linux_android = "$TOOLCHAIN\aarch64-linux-android21-clang.cmd"
$env:AR_aarch64_linux_android = "$TOOLCHAIN\llvm-ar.exe"

cargo build --release --target aarch64-linux-android
```

The resulting binary is `target\aarch64-linux-android\release\luna`
(no `.exe` extension — it is an Android ELF executable, not a Windows
one). `$env:RUSTFLAGS = ""` is set explicitly for the same reason
`scripts/build_release.ps1` sets it for the Windows build: so nothing
in `.cargo\config.toml`, or in whatever shell profile launched this
PowerShell session, can silently change what gets compiled in.

## Verifying the result

Using `llvm-readelf` from the same NDK
(`$TOOLCHAIN\llvm-readelf.exe`):

```powershell
& "$TOOLCHAIN\llvm-readelf.exe" -h target\aarch64-linux-android\release\luna
& "$TOOLCHAIN\llvm-readelf.exe" -d target\aarch64-linux-android\release\luna
& "$TOOLCHAIN\llvm-readelf.exe" -n target\aarch64-linux-android\release\luna
```

Expected:
- `-h`: `Type: DYN (Shared object file)`, `Machine: AArch64`.
- `-d`: exactly three `NEEDED` entries — `libc.so`, `libm.so`,
  `libdl.so`.
- `-n`: a `.note.android.ident` note encoding API level **21** and
  NDK release **r27d** (build **13750724**).
- File size: about **6.95 million bytes** (6.6 MiB).

## Honest caveats

- This build is **not byte-for-byte reproducible**: the compiler
  embeds absolute build paths, which differ from one machine to the
  next. The `SHA256SUMS` file published with each release is the only
  authoritative reference for "is this the exact file we shipped" —
  do not expect to reproduce its hash yourself from a different
  machine or directory layout.
- The resulting binary **cannot be run on a PC** (Windows or Linux):
  it is compiled for Android/ARM64 only. Functional testing requires
  copying it to an actual Android phone or tablet and running it from
  a UCI-capable chess GUI app there.
- The exact recipe used to build earlier releases (v3.1.7, v4.0.0,
  v4.1.0) was never written down anywhere in this repository. This
  document's recipe was reconstructed from scratch and verified only
  indirectly: by building v4.1.1 with it and confirming the result's
  `.note.android.ident` (API level, NDK release and build number) and
  `NEEDED` list match what the published v4.1.0 Android asset itself
  reports under the same tools (the note is identical byte for byte). It has not been
  cross-checked against any original build log, because none was kept in this repository.
