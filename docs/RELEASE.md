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

## 3. Assemble

```powershell
$version = (Select-String -Path Cargo.toml -Pattern '^version = "(.+)"').Matches[0].Groups[1].Value
$out = "dist\laukofront-v$version"
Remove-Item -Recurse -Force $out -ErrorAction SilentlyContinue
New-Item -ItemType Directory -Force $out | Out-Null
Copy-Item target\release\laukofront.exe $out
Copy-Item -Recurse assets $out\assets
Copy-Item LICENSE $out
Copy-Item NOTICE.md $out
Copy-Item docs\CREDITS.md $out
# a short player README (how to run, controls, Smart App Control note)
Copy-Item dist-README.txt $out\README.txt
```

Do **not** copy `target\`, `reference\`, `docs\` (except CREDITS), `src\` or `.git\`.

## 4. Test the packaged copy

1. Copy the folder somewhere outside the project, e.g. `C:\Temp\oc-test\`.
2. Run `laukofront.exe` there by double-clicking it (not `cargo run`).
3. Play to the first capture, quit, check no error dialogs.

## 5. Zip

```powershell
Compress-Archive -Path $out -DestinationPath "dist\laukofront-v$version.zip" -Force
```

## 6. Tag

```powershell
git tag "v$version"
```

## Known limitation: unsigned exe

The exe is not code-signed. PCs with Windows Smart App Control on may block it, and SmartScreen
may warn on first run ("More info → Run anyway"). Code signing is out of scope for this project.

`dist-README.txt` is written in Stage 17.
