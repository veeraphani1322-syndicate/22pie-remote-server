# 22Pie Remote Server — Phase 1

Central server foundation for authorized remote administration of computers you own or have explicit permission to manage. Phase 1 provides REST health/device APIs, an agent WebSocket, registration, heartbeat monitoring, and in-memory online/offline tracking.

It intentionally does **not** include remote control, screen capture, command execution, file transfer, unattended persistence, WebRTC, authentication, databases, or public internet exposure.

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
