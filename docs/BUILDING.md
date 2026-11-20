# Building

Exact steps from a clean Windows 10/11 PC, in PowerShell.

## 1. Install tools

1. **Visual Studio Build Tools** (the MSVC linker Rust needs on Windows):
   ```powershell
   winget install Microsoft.VisualStudio.2022.BuildTools --override "--quiet --wait --add Microsoft.VisualStudio.Workload.VCTools --includeRecommended"
   ```
2. **Rust** via rustup:
   ```powershell
   winget install Rustlang.Rustup
   ```
   Close and reopen PowerShell afterwards.
3. **Git**:
   ```powershell
   winget install Git.Git
   ```

The project pins its toolchain in `rust-toolchain.toml` (Rust 1.99.1 with clippy and rustfmt).
rustup downloads it automatically the first time you run `cargo` in the folder.

## 2. Windows Smart App Control

If **Smart App Control** is on (Windows Security → App & browser control → Smart App Control
settings), Windows blocks the unsigned helper programs cargo compiles, and the build fails with
`An Application Control policy has blocked this file. (os error 4551)`. It has to be **Off** to
build. On many Windows 11 builds it cannot be turned back on without resetting Windows; decide
before switching.

## 3. Get the code

```powershell
git clone <repository-url> laukofront
Set-Location laukofront
```

## 4. Build and test

```powershell
cargo build              # first build compiles Bevy: several minutes
cargo test
cargo clippy --all-targets -- -D warnings
```

## 5. Run

```powershell
cargo run                              # the game (Stage 3: an empty app that exits at once)
cargo run --release --bin sim_bench    # headless simulation benchmark
```

Always run from the project root so `assets/` is found.

## 6. Release build

```powershell
cargo build --release
```

Output: `target\release\laukofront.exe`. Packaging: `docs/RELEASE.md`.

## Troubleshooting

| Symptom | Fix |
|---|---|
| `link.exe not found` | install the Build Tools (step 1.1) |
| `os error 4551` | turn Smart App Control off (step 2) |
| `cannot read assets/config/game.toml` | run from the project root |
| `bad config in ...: missing field ...` | the named key is missing from the TOML file |
