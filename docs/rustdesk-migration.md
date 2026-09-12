# RustDesk feature migration into 22Pie

Target: replace the Graphic Services client with the 22Pie Windows agent and
browser dashboard, retaining VPS 8.234.114.242. This is an implementation backlog,
not a claim of RustDesk compatibility or completed feature parity.

## Current implementation

Device registration, identity keys, authenticated browser access, heartbeat,
reconnect, primary-monitor H.264 screen viewing, mouse and keyboard control,
per-permission consent, trusted accounts, revocation, and WebRTC statistics exist.
They still require Windows-to-browser testing across separate networks. Server
unit tests alone do not establish end-to-end reliability.

The release agent now uses a visible console with Ctrl+C shutdown. Login startup
remains opt-in. Background/unattended access must never be silently enabled by
installation. A future native status window should show connectivity, active
permissions, session termination, and settings without depending on Flutter.

## Feature completion criteria

| Feature | Current state | Acceptance criteria |
|---|---|---|
| Screen and input | Implemented, Windows verification pending | View changing desktop; click/type; reconnect; release held keys on disconnect |
| File transfer | Browser-to-Windows implemented; Windows validation and reverse direction pending | Both directions, folder selection, progress, cancellation, size limits, integrity checks, traversal and overwrite protection |
| Clipboard | Missing | Explicit permission, bidirectional text then files/images; prevent feedback loops and stale-session delivery |
| Audio | Missing | Windows system audio and microphone with independent permission, mute, codec negotiation and cleanup |
| Multiple monitors | Missing | Enumerate/select displays; map input coordinates; handle unplug and resolution changes |
| Quality and codecs | Basic 720p/15 FPS | Adaptive quality, hardware acceleration, fallback, correct aspect ratio and metrics |
| Chat | Missing | Session-scoped messages with bounded payloads and clear sender identity |
| Recording | Missing | Visible recording state, start/stop, usable download and cleanup |
| Address book/device groups | Basic device list | Search, aliases, groups and per-user ownership |
| Unattended access | Trusted sessions while agent runs | Explicit opt-in setup, revocation, visible status, no installation-time activation |
| Elevated/UAC control and reboot | Missing | Local authorization, signed packaging, reconnect after reboot, no privilege bypass |
| Remote printing | Missing | Driver packaging, user-selected printer, spool cleanup and uninstall |
| TCP tunneling | Missing | Explicit target/port authorization, ownership, bounded connections and disconnect cleanup |
| Privacy/virtual displays | Missing | Explicit local opt-in, reliable emergency exit, restore displays on crash |
| Remote terminal | Missing | Explicit permission, authenticated session binding, visible state and process cleanup |
| Mobile/macOS/Linux parity | Missing beyond identity/heartbeat | Platform-specific capture/input/consent, packaging and actual-device validation |

Implementation order: verified Windows foreground screen/control baseline; secure
relay; file transfer/clipboard/chat; monitors/audio/recording; explicitly enabled
advanced access and platform-specific features. Preserve separate permissions:
existing screen/input trust must not silently authorize new capabilities.

## Port inventory and cutover

The checked-in Graphic Services service runs hbbs with relay
`8.234.114.242:21117`. There is no checked-in live listener snapshot. Confirm actual
VPS listeners before changing anything.

| Existing RustDesk port | Existing purpose | 22Pie migration treatment |
|---|---|---|
| TCP 21115 | NAT test | Reserved; no equivalent listener implemented |
| TCP/UDP 21116 | Rendezvous/registration | Profile uses TCP 21116 for 22Pie HTTP/WebSocket API; UDP reserved |
| TCP 21117 | RustDesk relay | Planned authenticated TURN endpoint; NOT implemented by hbbr |
| TCP 21118 | Rendezvous WebSocket | Profile uses HTTP dashboard on 21118 |
| TCP 21119 | Relay WebSocket | Reserved; no equivalent listener implemented |

22Pie speaks WebRTC, not RustDesk protobuf. Reusing a port number does not make
hbbs/hbbr serve 22Pie sessions. A TURN server is needed for relay; its media
allocation port range and firewall policy must also be configured. Do not point
ICE_SERVERS at hbbr and expect it to work. HTTPS/WSS and TURN configuration remain
deployment work; the supplied examples are development profiles only.

Profiles live in `deploy/graphic-services/`. They do not change the current 4000
fallback or mutate the VPS. For a planned cutover:

1. Back up the current device/trust stores and server configuration. Record active
   listeners, running services, firewall rules and rollback commands.
2. Test 22Pie using separate ports while Graphic Services remains available.
3. Prepare server secrets, HTTPS/WSS and authenticated TURN. Validate forced relay
   between separate networks before retiring the old client.
4. Stop conflicting hbbs/hbbr listeners during the cutover. Copy the server profile
   to the server `.env`, preserving real secrets and store paths. The server listens
   on 21116. Copy the dashboard profile to `.env.local`, build the dashboard, then
   run `npm start -- --hostname 0.0.0.0 --port 21118` from `dashboard/`.
5. Merge the example agent config into the user's existing config without deleting
   device identity. Restart the agent. The new server URL changes the trust scope;
   require fresh approval rather than copying prior trust to the new endpoint.
6. Verify login, registration, live screen, mouse/keyboard, denial, revocation,
   disconnect and Ctrl+C. Roll back if the actual Windows test fails.

Reference: https://github.com/rustdesk/rustdesk-server/blob/master/docs/environment-variables.md

## File upload increment

The updated Windows agent offers a session-bound `files-v1` WebRTC data channel.
The dashboard sends one file at a time, at most 32 MiB, in acknowledged 16 KiB
chunks. It uses bundled SHA-256 and browser getRandomValues, including on the existing HTTP dashboard. HTTPS/WSS remains recommended for protecting dashboard login and signaling. Each offer prompts
on Windows for up to 60 seconds; denial, cancellation, and disconnect grant no
future permission. Remote mouse/keyboard events are suppressed while this local
approval window is open.

Files go into `Downloads/22Pie Transfers/<transfer UUID>/`. The receiver rejects
paths, Windows reserved names, out-of-order chunks and size mismatches. It writes
an incomplete file, verifies SHA-256, and exposes the final name without replacing
an existing file. Cancelled/failed transfers delete partial files. Received files
are marked with Windows' Internet zone and are never executed or opened.

Limits: upload only; no remote browsing, folder uploads, resume, or download from
Windows yet. Ten offers are allowed per session; an active transfer idle for 90
seconds closes its file channel and requires reconnecting. Each browser file is
buffered up to the 32 MiB limit. A disconnect during final confirmation can leave
a successfully saved file without confirmation at the sender; check the remote
Downloads folder before retrying. Native Windows execution and two-network tests
remain required before release.

## VPS deployment — 2026-09-12

Release `f06bf78` is deployed under `/home/twilightkart/22pie-releases/f06bf78` on `8.234.114.242`. The original checkout, administrator credentials and device/trust data remain in place. The release references the original persistent data paths.

User systemd services `22pie-server` and `22pie-dashboard` listen on TCP 21116 and 21118. Compatibility sockets `22pie-compat-api` and `22pie-compat-dashboard` forward TCP 4000 and 3000 respectively, preserving existing client defaults and dashboard bookmarks. User lingering is enabled. Old `graphic-services-hbbs` and `graphic-services-hbbr` services are stopped and disabled, with their files retained for rollback. Other former RustDesk ports are not yet used by 22Pie; reusing port numbers does not provide RustDesk protocol or feature compatibility.

Dashboard: `http://8.234.114.242:21118/login` (existing port 3000 also works). Agent: `ws://8.234.114.242:21116/agent`. HTTPS/WSS migration and full RustDesk feature parity remain outstanding. The portable Windows x64 artifact comes from successful Actions run `34681940355`, agent revision `68393c1`; the subsequent release only changes dashboard dependencies and upload hashing. The supplied launcher sets the VPS endpoint, Graphics Services display names, and `START_WITH_WINDOWS=false`.

Verified: external HTTP 200 on both dashboard and API ports, authenticated device listing, rejection of anonymous device access, CORS and agent WebSocket upgrade. Actual Windows screen/control/file transfer still requires a connected Windows computer.
