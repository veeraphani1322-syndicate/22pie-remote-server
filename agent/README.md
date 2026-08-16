# Graphic Service — 22Pie authorized remote agent

This Rust agent authenticates an authorized computer, registers it with the 22Pie server, sends heartbeats, reconnects with capped exponential backoff, and supports consent-gated live viewing of the Windows primary monitor. It does not contain mouse/keyboard control, command execution, persistence, surveillance, clipboard access, or file-transfer capabilities.

## Phase 3 screen sharing

An untrusted viewing request displays a native Windows dialog with **Allow Once**, **Trust This Account**, and **Deny**. Trust is scoped to this device identity, server URL, stable account ID, and `SCREEN_VIEW`; both the server and agent must have matching records before a later prompt is skipped. While sharing, a visible top-level 22Pie Remote indicator remains open; closing it stops capture and ends the viewer session. Only one screen request can be pending or active at a time.

Trusted access is stored at `%LOCALAPPDATA%\22Pie\RemoteAgent\trusted-access.json`. Screen, mouse, and keyboard trust can be revoked independently from the authenticated dashboard. First-time permissions retain explicit consent; trusted sessions do not show repeated activity dialogs.

The capture pipeline uses the primary monitor, scales conservatively to at most 1280×720, limits output to approximately 15 FPS, encodes H.264, and sends it through WebRTC rather than JSON screenshots. ICE configuration arrives over the authenticated agent channel, supporting direct STUN negotiation and TURN relay without embedding TURN secrets in the executable.

The existing UUID is now paired with an Ed25519 device key in `device.json`. Registration uses a fresh server challenge and signature. The private key remains on the endpoint and is never sent to the server.

The development build connects to `ws://8.234.114.242:4000/agent` by default. Only use it on computers you own or have explicit permission to manage.

## Configuration

The server URL is selected in this order:

1. The `REMOTE_SERVER_URL` environment variable.
2. `config.json` in the per-user application data directory.
3. The compiled development default, `ws://8.234.114.242:4000/agent`.

On Windows, the application directory is `%LOCALAPPDATA%\22Pie\RemoteAgent`. An optional configuration file at `%LOCALAPPDATA%\22Pie\RemoteAgent\config.json` looks like this:

```json
{
  "remoteServerUrl": "wss://remote.22pie.com/agent",
  "appDisplayName": "Graphic Service",
  "windowTitle": "Graphic Service",
  "trayDisplayName": "Graphic Service",
  "startWithWindows": false
}
```

The optional `HEARTBEAT_INTERVAL_SECONDS` environment variable changes the 20-second heartbeat interval. It must be between 5 and 3600 seconds. A local `.env` file is also supported for development and is ignored by Git.

## Stable device identity

On first launch, the agent detects the hostname, operating-system version, and architecture and generates a UUID. On Windows it stores the UUID in:

```text
%LOCALAPPDATA%\22Pie\RemoteAgent\device.json
```

Future launches by the same Windows user reuse that UUID. If the file is unreadable or invalid, the agent reports an error instead of silently changing identity. Delete `device.json` only when you intentionally want the machine to register as a new device.

## Windows Standalone Build

The release executable is a single background Windows GUI-subsystem application. It opens no console, remains visible as `GraphicService.exe` in Task Manager, and uses no watchdog or child product executable.

### Build locally on Windows

Install stable Rust with the MSVC toolchain on a 64-bit Windows development computer, open PowerShell in the `agent` directory, and run:

```powershell
rustup default stable-msvc
cargo build --release --locked
```

The executable is generated at:

```text
target\release\GraphicService.exe
```

Copy the executable to an authorized Windows 10/11 computer and double-click it. Use Task Manager to stop it. See `README-Windows.txt` for startup-at-login and migration instructions.

### Build with GitHub Actions

The repository includes `.github/workflows/build-windows-agent.yml`, which builds the `x86_64-pc-windows-msvc` release on `windows-latest` without using secrets.

1. Push the changes to GitHub.
2. Open the repository's **Actions** tab.
3. Select **Build Windows Agent**.
4. Choose **Run workflow**, select the branch, and run it.
5. Open the completed workflow run.
6. Download the **GraphicService-Windows** artifact.
7. Extract `GraphicService.exe` and `README.txt` and copy them to the authorized Windows laptop.

The workflow also runs automatically when agent files or the workflow itself change. macOS-to-MSVC cross-compilation needs a compatible Microsoft linker and Windows SDK and is not configured here; GitHub's Windows runner is the reliable cross-platform path.

## Run and verify

Double-click `GraphicService.exe`. The production build has no console output. Verify startup in Task Manager, the dashboard, and `%LOCALAPPDATA%\22Pie\RemoteAgent\logs\GraphicService.log`.

```text
22Pie Remote Agent v0.1.1

Device Name:
OFFICE-LAPTOP

Operating System:
Windows 11 ...

Server:
ws://8.234.114.242:4000/agent

Connecting...
Connected
Registered successfully

Status:
ONLINE

Press Ctrl+C to stop.
```

Verify registration from another computer by opening:

```text
http://8.234.114.242:4000/api/devices
```

The device should be `online`. Test again with the Windows laptop on a separate network such as a mobile hotspot. After Ctrl+C, the server should mark it `offline`.

The agent makes an outbound WebSocket connection and does not change Windows Firewall. If Windows asks whether to allow network access during authorized testing, permit the network type you are testing on; do not disable the firewall.

This development executable is unsigned. Microsoft Defender SmartScreen may show **Windows protected your PC** or another warning. Inspect and trust only artifacts built from this repository. Production distribution should use a proper code-signing certificate; this project does not bypass SmartScreen or Defender.

## Development validation

From the `agent` directory:

```bash
cargo fmt --check
cargo check
cargo build
```

Set `RUST_LOG=debug` to display successful heartbeat acknowledgements. If connection fails, the console reports it and retries after 1, 2, 4, 8, 16, then at most 30 seconds until it connects or the user stops it.
