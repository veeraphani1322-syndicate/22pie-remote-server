# Graphics Services Windows 0.1.2

First Windows-only implementation increment against the DWService audit. Deliver one `GraphicService.exe`; no CMD launcher, sidecar application DLL, Node.js, or Rust installation. Normal per-user identity/configuration/log data is still stored under `%LOCALAPPDATA%/22Pie/RemoteAgent`. Windows system DLLs remain required.

## Changes

- Defaults directly to the current VPS API on port 21116. Preserves explicit endpoint overrides and existing identity/trust records.
- Defaults to Graphics Services display names.
- High streaming profile: up to 1920×1080 or portrait equivalent, target 30 FPS, 8 Mbps. Balanced retains 720p/15 FPS/2 Mbps; native permits up to 4K/30 FPS/20 Mbps. Never upscale; enforce even H.264 dimensions and reject degenerate inputs.
- Hamming downscaling replaces nearest-neighbor resizing when dimensions change. Encoding remains software OpenH264; actual FPS depends on host CPU/network.
- `STREAM_QUALITY` or optional `streamQuality` in existing configuration selects balanced/high/native. No Auto mode or browser quality selector yet.
- Enable certificate-verified WSS using the OS trust store. Existing VPS still uses WS/HTTP: this build does not deploy TLS to the server.
- Bound connection attempts to 15 seconds and make pending connections cancellable by Ctrl+C.
- Windows build statically links the CRT, checks for unwanted Visual C++ runtime imports, and runs `--version` from an isolated folder containing just the EXE. The artifact includes only the executable.

## Scope and verification

Windows x64 is the only current delivery target. The source still contains non-Windows guards for local tests; this is not Linux/macOS product support.

Local tests cover quality dimensions, portrait/ultrawide scaling, invalid inputs and existing protocol/permission/file regressions. Windows CI checks and tests the Windows code, builds the release, validates imports and checks standalone process startup. Actual-device visual quality, CPU use and remote session latency require Windows testing; do not claim they passed from CI alone.

This partially addresses NET-01, NET-07, SCR-02 and EXT-01. None passes the complete DWService acceptance gate yet. Proxy transport, authenticated session relay, owner-bound enrollment, native configuration UI, installed unattended service, hardware encoding and adaptive quality remain outstanding. Startup at login remains opt-in; installing a service is not part of this release.
