# DWService parity audit — baseline 2026-09-12

The target is DWAgent/DWService functional parity under Graphics Services branding, retaining 22Pie and the existing Google VPS. This is the first public-documentation and source audit, not a certified exhaustive inventory or a completed implementation. No row is end-to-end verified. The user has reported that the existing stream works but its quality is poor.

Baseline: 22Pie commit `890723c`; DWAgent source commit `0ee89fdb72ae1341516b560ef80a2cbdf26a9d2f`. Live documentation was reviewed on 2026-09-12. No DWService authenticated account, native settings walkthrough, or reference-device testing was available in this audit. Those gaps remain explicitly tracked, including account/billing/API screens and platform exceptions. Windows x64 remains the first delivery platform; other documented platforms remain on the parity backlog.

## Implementation updates

Windows x64 is the only current delivery target, supplied as one executable (user scope update). [Version 0.1.2](windows-0.1.2.md) adds client TLS and manual quality profiles; NET-01 and SCR-02 now have partial implementation. The counts below describe the original audit baseline; the CSV tracks current progress. No new end-to-end parity gate has passed.

## Inventory

There are 64 tracked requirements: 43 missing, 17 partial, 4 unverified. “Partial” means related implementation exists, not that it meets the reference behavior. “Missing” means no implementation of the required behavior was found in the inspected 22Pie code. “Unverified” requires more reference discovery. Acceptance tests are proposed 22Pie tests, not claims about undocumented DWService behavior.

The [CSV inventory](dwservice-feature-inventory.csv) includes a reference URL, local code evidence, delivery phase and acceptance test for every row. Use stable IDs in implementation PRs and test reports. New reference discoveries add rows; they must not be silently dropped to reach 100%.

## Findings that change the implementation plan

- **Proxy:** reference source distinguishes direct, system, HTTP CONNECT, SOCKS4, SOCKS4A and SOCKS5. Our agent calls `connect_async` directly. A proxy setting for registration alone will not carry WebRTC media through an HTTP proxy. Whole-session proxy tests are required.
- **TLS:** `agent/Cargo.toml` has no TLS feature for tokio-tungstenite. `cargo tree -e features -i tokio-tungstenite --locked` confirms only the default connect/handshake/stream features. Merely changing the endpoint to `wss://` is insufficient. A rustls dependency elsewhere in Cargo.lock does not enable WebSocket TLS.
- **Enrollment:** `src/websocket/agent-server.ts` currently registers authenticated device keys with the literal owner `admin`. Proof of a device key is not an enrollment authorization. Add owner-bound enrollment before multi-user sharing.
- **Quality:** `agent/src/media.rs` limits capture to 1280×720, 15 FPS, 2 Mbps, nearest-neighbor resizing and software OpenH264. Reference quality includes manual levels and Auto; a static larger bitrate would still leave Auto absent.
- **Unattended:** our Run-key startup only runs after user login. Trusted screen/input grants while the process runs do not provide pre-login, logout, secure-desktop or reboot support. Build an explicit service plus interactive-session helper.
- **Permissions:** the current initial session bundles screen, mouse and keyboard. Reference view-only sharing needs independent capabilities. Existing trust must not automatically grant terminal, file, audio or service-control access.
- **Files:** our upload is limited to one approved file at a time, 32 MiB, under Downloads. It is not the reference file manager. Local hash checks do not verify real Windows upload behavior.
- **Printing:** DWService documents printing as a workaround using file transfer and local/remote printing; it explicitly says native remote printing is not yet available. Do not equate that with a shipped printer-redirection driver.

See [the implementation architecture and delivery gates](dwservice-architecture.md) for the proposed transport and service changes.

## Requirement table

| ID | Capability | 22Pie status | Phase |
|---|---|---|---|
| NET-01 | HTTPS and certificate-verified WSS | Missing | P1 |
| NET-02 | Session relay through server nodes | Missing | P1 |
| NET-03 | HTTP CONNECT proxy and credentials | Missing | P1 |
| NET-04 | SOCKS4 and SOCKS4A | Missing | P1 |
| NET-05 | SOCKS5 and username/password | Missing | P1 |
| NET-06 | System proxy selection and explicit direct mode | Missing | P1 |
| NET-07 | Disconnect recovery and heartbeat | Partial | P1 |
| NET-08 | Node routing and bandwidth management | Missing | P1 |
| AGT-01 | Account enrollment and installation code | Missing | P1 |
| AGT-02 | Portable run-only session | Partial | P2 |
| AGT-03 | Native status and configuration interface | Missing | P2 |
| AGT-04 | Installed Windows service and uninstall | Missing | P2 |
| AGT-05 | Unattended access toggle | Partial | P2 |
| AGT-06 | Unattended operation through logout and reboot | Missing | P2 |
| AGT-07 | Pause and resume agent | Partial | P2 |
| AGT-08 | Configuration access password | Missing | P2 |
| AGT-09 | Automatic update and reinstall | Missing | P2 |
| AGT-10 | Silent installation and managed deployment | Missing | P2 |
| AGT-11 | Active-session monitor and notifications | Partial | P2 |
| AGT-12 | Device rename grouping and organization | Partial | P5 |
| SCR-01 | Screen capture and keyboard/mouse | Partial | P3 |
| SCR-02 | Minimum Low Medium Maximum Auto quality | Missing | P3 |
| SCR-03 | Zoom percentage and fit-to-window | Partial | P3 |
| SCR-04 | Fullscreen toolbar | Partial | P3 |
| SCR-05 | Multiple monitor selection and combined layout | Missing | P3 |
| SCR-06 | View-only mode | Partial | P3 |
| SCR-07 | Clipboard copy and paste | Missing | P4 |
| SCR-08 | Special key actions and Ctrl+Alt+Del | Partial | P2 |
| SCR-09 | Remote audio and mute | Missing | P4 |
| SCR-10 | Connection statistics | Partial | P3 |
| SCR-11 | Adaptive screen encoding | Missing | P3 |
| FIL-01 | Directory and drive browsing | Missing | P4 |
| FIL-02 | Upload to selected location | Partial | P4 |
| FIL-03 | Download from remote device | Missing | P4 |
| FIL-04 | Create directories and rename | Missing | P4 |
| FIL-05 | Copy and move | Missing | P4 |
| FIL-06 | Delete files and folders | Missing | P4 |
| FIL-07 | File permissions | Missing | P4 |
| FIL-08 | Folder-scoped operation permissions | Missing | P4 |
| RES-01 | System CPU memory disk information | Missing | P4 |
| RES-02 | Process listing and termination | Missing | P4 |
| RES-03 | Service listing start and stop | Missing | P4 |
| SHL-01 | Interactive terminal session | Missing | P4 |
| EDT-01 | Read and save text files | Missing | P4 |
| LOG-01 | Follow a remote log | Missing | P4 |
| ACC-01 | Login logout and account lifecycle | Partial | P1 |
| ACC-02 | TOTP and recovery codes | Missing | P5 |
| ACC-03 | Trusted browser devices | Missing | P5 |
| ACC-04 | Contacts and groups | Missing | P5 |
| ACC-05 | Activity and access history | Partial | P5 |
| SHR-01 | Per-application shares | Missing | P5 |
| SHR-02 | Share credentials and expiry | Missing | P5 |
| SHR-03 | Read-only file and screen access | Missing | P5 |
| PLT-01 | Windows support matrix | Partial | P2 |
| PLT-02 | Linux GUI console and desktop sessions | Missing | P6 |
| PLT-03 | macOS agent and system permissions | Missing | P6 |
| PLT-04 | Browser and touch interactions | Partial | P6 |
| PLT-05 | Localization | Missing | P6 |
| PRN-01 | Remote printing workflow | Missing | P4 |
| API-01 | Public API account agent session shares | Unverified | P5 |
| BIZ-01 | Subscriptions quotas invoices and account settings | Unverified | P5 |
| UI-01 | Every authenticated screen menu and setting | Unverified | P0 |
| EXT-01 | Hardware encoding and high-resolution performance | Missing | P3 |
| EXT-02 | RustDesk-only features retained from earlier request | Unverified | P0 |

## Primary sources

- [index](https://docs.dwservice.net/)
- [network](https://www.dwservice.net/en/security.html)
- [proxy](https://github.com/dwservice/agent/blob/0ee89fdb72ae1341516b560ef80a2cbdf26a9d2f/core/communication.py)
- [agent](https://github.com/dwservice/agent/blob/0ee89fdb72ae1341516b560ef80a2cbdf26a9d2f/core/agent.py)
- [screen](https://docs.dwservice.net/docs/site/basic-definitions/screen-application/)
- [capture](https://github.com/dwservice/agent/blob/0ee89fdb72ae1341516b560ef80a2cbdf26a9d2f/app_desktop/desktop.py)
- [files](https://github.com/dwservice/agent/blob/0ee89fdb72ae1341516b560ef80a2cbdf26a9d2f/app_filesystem/filesystem.py)
- [resources](https://github.com/dwservice/agent/blob/0ee89fdb72ae1341516b560ef80a2cbdf26a9d2f/app_resource/resource.py)
- [shell](https://github.com/dwservice/agent/blob/0ee89fdb72ae1341516b560ef80a2cbdf26a9d2f/app_shell/shell.py)
- [editor](https://github.com/dwservice/agent/blob/0ee89fdb72ae1341516b560ef80a2cbdf26a9d2f/app_texteditor/texteditor.py)
- [logs](https://github.com/dwservice/agent/blob/0ee89fdb72ae1341516b560ef80a2cbdf26a9d2f/app_logwatch/logwatch.py)
- [share](https://docs.dwservice.net/docs/site/agent-sharing/how-do-i-share-an-agent-with-restrictions/)
- [unattended](https://docs.dwservice.net/docs/site/basic-definitions/unattended-access/)
- [run](https://docs.dwservice.net/docs/site/dwservice-installation/how-do-i-run-only-the-agent/)
- [print](https://docs.dwservice.net/docs/site/workarounds/%F0%9F%96%A8%EF%B8%8F-remote-printing/)
- [2fa](https://docs.dwservice.net/docs/site/account-management/how-to-enable-2-factor-authentication/)
