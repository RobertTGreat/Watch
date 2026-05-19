release:
    cargo build --release

dist:
    cargo build --profile dist
    powershell -NoProfile -ExecutionPolicy Bypass -File installer\build-installer.ps1 -Configuration dist -SkipCargoBuild
