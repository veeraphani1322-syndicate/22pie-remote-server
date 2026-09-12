# 22Pie Remote Server — Phase 3

Central server for authorized remote support on computers you own or have explicit permission to manage. It provides authenticated device APIs, an agent WebSocket, registration, heartbeat monitoring, and online/offline tracking.

Phase 3 adds an authenticated Next.js dashboard, persistent per-device Ed25519 credentials, owner-filtered device access, explicit Windows Allow/Deny consent, WebRTC signaling, primary-monitor H.264 capture with manual balanced/high/native profiles (default up to 1080p, targeting 30 FPS), configurable STUN/TURN, visible sharing status, session timeouts, and actual browser WebRTC statistics. See [`docs/phase-3-deployment.md`](docs/phase-3-deployment.md) for deployment and cross-network validation.

Mouse and keyboard control and permission-scoped trusted access are implemented. The Windows agent opens a visible console and does not enable startup at login by default. Browser-to-Windows file upload now has per-file approval, progress, cancellation, and integrity checks (Windows validation pending). Clipboard, reverse file transfer, audio, and the remaining RustDesk feature migration are tracked in [`docs/rustdesk-migration.md`](docs/rustdesk-migration.md).

## DWService feature target

The current target is DWAgent/DWService functional parity. See the [reference audit](docs/dwservice-parity-audit.md), [feature inventory with acceptance tests](docs/dwservice-feature-inventory.csv), and [implementation architecture](docs/dwservice-architecture.md). These track missing work; the current release is not feature-complete. Current delivery scope is Windows x64 as one `GraphicService.exe`; Linux/macOS are deferred. See [Windows 0.1.2 changes](docs/windows-0.1.2.md).

## Requirements

- Node.js 20 or newer

## Start the server

```bash
npm install
npm run dev
```

The server listens on `0.0.0.0:4000` by default. Copy `.env.example` to `.env` to customize local configuration.

## Try Phase 1

Health: <http://localhost:4000/api/health>

Devices: <http://localhost:4000/api/devices>

In a second terminal, run:

```bash
npm run test:agent
```

The device endpoint will show:

```text
TEST-LAPTOP
🟢 online
```

Stop the simulator with Ctrl+C. The device remains known in memory and changes to:

```text
TEST-LAPTOP
offline
```

Device state is intentionally lost whenever the server restarts in Phase 1.

## Commands

```bash
npm run dev         # development server with reload
npm run build       # compile TypeScript into dist/
npm start           # run the compiled server
npm run test:agent  # run the development-only agent simulator
npm run typecheck   # validate TypeScript without emitting files
```

## API and protocol

- `GET /api/health`
- `GET /api/devices`
- `GET /api/devices/:deviceId`
- `ws://localhost:4000/agent`

An agent must register with a valid UUID before sending heartbeats. Incoming messages are strictly validated and limited in size. If the same device ID registers again, the newest connection replaces the old connection without allowing the old socket to alter current device state.

For local development, browser CORS is restricted to `http://localhost:3000` by default. Set `CORS_ORIGIN` to a comma-separated allowlist when needed.

## Phase 2 Rust agent

The real lightweight desktop agent lives in [`agent/`](agent/). It collects the host identity, persists a stable device UUID, registers, sends heartbeats, reconnects automatically, and handles Ctrl+C cleanly. See [`agent/README.md`](agent/README.md) for macOS/Windows setup and LAN testing instructions.
