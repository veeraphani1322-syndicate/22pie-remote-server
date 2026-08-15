# Phase 3 deployment and test guide

Phase 3 adds authenticated, consent-gated screen viewing. It does not add mouse, keyboard, clipboard, file, terminal, service, installer, or unattended-access capabilities.

## VPS server configuration

Pull the Phase 3 commit, install dependencies, and build:

```bash
npm ci
npm run test
npm run build
```

Create a password hash locally. Do not put the plaintext password in `.env`:

```bash
npm run hash-password -- "choose a long unique password"
```

Configure the server using `.env.example`. At minimum, replace these values:

```dotenv
JWT_SECRET=<at-least-32-random-characters>
ADMIN_EMAIL=you@example.com
ADMIN_PASSWORD_HASH=<argon2id-output>
PUBLIC_BASE_URL=http://8.234.114.242:4000
CORS_ORIGIN=http://8.234.114.242:3000
DEVICE_STORE_PATH=data/devices.json
TRUST_STORE_PATH=data/trusted-access.json
ICE_SERVERS=[{"urls":["stun:stun.l.google.com:19302"]}]
```

For cross-network reliability, deploy coturn and add a TURN entry. Keep credentials only in the VPS environment, never in source code:

```dotenv
ICE_SERVERS=[{"urls":["stun:stun.l.google.com:19302"]},{"urls":["turn:turn.example.com:3478"],"username":"temporary-user","credential":"temporary-password"}]
```

Static TURN credentials are development-only. Use short-lived credentials before broader deployment. Phase 3 still uses plain HTTP/WS at the development IP; deploy TLS/WSS before public production use.

Start the server:

```bash
npm start
```

The persistent device credential registry and trusted-access repository are written with owner-only permissions under `data/` by default. Back them up securely. Trust records are bound to the authenticated user, device ID, device public key, and `SCREEN_VIEW` permission.

## Dashboard startup

On the VPS:

```bash
cd dashboard
npm ci
cp .env.example .env.local
```

For the current development host, set:

```dotenv
NEXT_PUBLIC_API_URL=http://8.234.114.242:4000
NEXT_PUBLIC_WS_URL=ws://8.234.114.242:4000
```

Then build and start:

```bash
npm run build
npm start -- --hostname 0.0.0.0 --port 3000
```

Allow TCP 3000 only for authorized development testing. The production design should reverse-proxy the dashboard and server through HTTPS/WSS on port 443.

## Windows agent

Download `22PieRemoteAgent-Windows` from the successful **Build Windows Agent** GitHub Actions run, extract `22PieRemoteAgent.exe`, and replace the previous development agent. The first Phase 3 run adds a private Ed25519 device key to `%LOCALAPPDATA%\22Pie\RemoteAgent\device.json`; the private key never leaves the endpoint. Preserve that file to preserve the enrolled identity.

## Exact cross-network test

1. Put the controlling laptop on Network A.
2. Put the remote Windows laptop on Network B, such as a mobile hotspot.
3. Start the VPS server and dashboard.
4. Run the new `22PieRemoteAgent.exe` on Windows and confirm it reports online.
5. Open `http://8.234.114.242:3000`, sign in, and confirm only owned devices are listed.
6. Select **View screen**. Confirm no capture starts yet.
7. On Windows, choose **Deny**. Confirm the viewer reports rejection and no video appears.
8. Request again and choose **Allow Once**. Confirm the session works and the next request prompts again.
9. Choose **Trust This Account**, disconnect, and confirm the next two sessions start without another consent dialog.
10. Revoke from the dashboard and confirm the next request displays consent again.
11. To revoke locally, type `revoke-trust` in the running agent console, press Enter, and confirm the visible trusted-access dialog.
12. Confirm live video appears, preserves aspect ratio, and updates when the Windows desktop changes.
13. Confirm resolution, FPS, bitrate, Network RTT, packet loss, Direct/Relay, and protocol values come from browser WebRTC statistics.
14. Select **Fullscreen**, then exit fullscreen.
15. Select **Disconnect** and confirm capture ends.
16. Repeat, then use the Windows sharing indicator to stop locally; confirm the browser session ends immediately.
17. Close the agent during a session and confirm the viewer session fails and cleans up.
18. Leave an approval request unanswered and confirm it expires after 60 seconds.
19. Confirm heartbeat/reconnect still works after restarting the agent.

Successful builds do not prove this test. Record browser, Windows version, networks, whether the selected ICE route was Direct or Relay, and any failures when performing it.
