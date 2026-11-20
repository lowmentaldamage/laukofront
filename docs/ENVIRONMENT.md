# Environment

Recorded 2026-11-20 (Stage 1). This is the development machine. Results measured here are from a
**high-end** PC; see `PERFORMANCE.md` for what that means for the "mid-range PC" target.

## Tools

| Tool | Version |
|---|---|
| rustc | 1.99.1 (58cf8f9c3 2026-10-13) |
| cargo | 1.99.1 (5f94df478 2026-08-27) |
| rustup | 1.29.3 (edbc831ce 2026-11-02) |
| Toolchain | stable-x86_64-pc-windows-msvc |
| git | 2.56.0.windows.4 |

## Machine

| Item | Value |
|---|---|
| OS | Microsoft Windows 11 Home 10.0.26300 (build 26300) |
| CPU | AMD Ryzen 9 7950X, 16 cores / 32 threads |
| GPU | NVIDIA GeForce RTX 4090, driver 32.0.16.2124 |
| RAM | 63.7 GB |
| Disk D: | 369.5 GB free, 96.3 GB used |

## How these were collected (PowerShell)

```powershell
rustc --version; cargo --version; rustup --version; git --version
Get-PSDrive D
Get-CimInstance Win32_OperatingSystem | Select Caption, Version, BuildNumber
Get-CimInstance Win32_Processor | Select Name, NumberOfCores, NumberOfLogicalProcessors
Get-CimInstance Win32_VideoController | Select Name, DriverVersion
```
