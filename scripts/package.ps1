# Builds the Windows release zip: dist\laukofront-<version>-windows.zip
# Run from the project root:  powershell -ExecutionPolicy Bypass -File scripts\package.ps1
$ErrorActionPreference = "Stop"
$version = (Select-String -Path Cargo.toml -Pattern '^version = "(.+)"').Matches[0].Groups[1].Value
$name = "laukofront-$version-windows"
$out = "dist\$name"

cargo build --release
if ($LASTEXITCODE -ne 0) { throw "build failed" }

if (Test-Path $out) { Remove-Item -Recurse -Force $out }
New-Item -ItemType Directory -Force "$out\assets\config" | Out-Null
Copy-Item target\release\laukofront.exe $out
Copy-Item assets\config\game.toml "$out\assets\config\"
Copy-Item LICENSE, NOTICE.md $out
Copy-Item docs\CREDITS.md, docs\CONTROLS.md $out
Copy-Item dist-README.txt "$out\README.txt"
# AGPL: ship the corresponding source (the committed tree at HEAD).
git archive --format=zip --output "$out\laukofront-source.zip" HEAD
if ($LASTEXITCODE -ne 0) { throw "git archive failed" }

$zip = "dist\$name.zip"
if (Test-Path $zip) { Remove-Item -Force $zip }
Compress-Archive -Path "$out\*" -DestinationPath $zip
Write-Host "Packaged $zip"
