# Release

How to make the Windows build and zip it (Stage 17). Run in PowerShell from the project root.

## 1. Check

```powershell
cargo test
cargo clippy --all-targets -- -D warnings
```

Both must pass. Update the version in `Cargo.toml` and `CHANGELOG.md` first.

## 2. Build

```powershell
cargo build --release
```

## 3. Package

```powershell
powershell -ExecutionPolicy Bypass -File scripts\package.ps1
```

This builds the release exe and writes `dist\laukofront-<version>-windows.zip` with the exe,
`assets\config\game.toml` (maps are built into the exe), `LICENSE`, `NOTICE.md`, `CREDITS.md`,
`CONTROLS.md`, `README.txt` (from `dist-README.txt`) and `laukofront-source.zip` (the committed
source at HEAD, as the AGPL requires). Commit first, so the source zip matches the exe.

## 4. Test the packaged copy

1. Unzip it somewhere outside the project.
2. Run `laukofront.exe` there (double-click). The main menu opens; start a match and play a bit.

## 5. Tag

```powershell
git tag "v$version"
```

## Known limitation: unsigned exe

The exe is not code-signed. PCs with Windows Smart App Control on may block it, and SmartScreen
may warn on first run ("More info -> Run anyway"). Code signing is out of scope for this project.
