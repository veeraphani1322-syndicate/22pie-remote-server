# 22Pie Remote Agent — Phase 2

This lightweight Rust agent identifies an authorized computer, registers it with the 22Pie server, sends heartbeats, reconnects with capped exponential backoff, and shuts down cleanly. It does not contain remote-control, command-execution, persistence, surveillance, or file-transfer capabilities.

## Requirements

- Rust stable toolchain: <https://rustup.rs>
- A running Phase-1 server reachable on the same trusted local network

## Configure and run

The agent defaults to `ws://localhost:4000/agent`. When it runs on another computer, set the Mac mini's LAN address instead.

macOS or Linux:

```bash
export REMOTE_SERVER_URL=ws://192.168.1.100:4000/agent
export HEARTBEAT_INTERVAL_SECONDS=20
cargo run
```

Windows PowerShell:

```powershell
$env:REMOTE_SERVER_URL="ws://192.168.1.100:4000/agent"
$env:HEARTBEAT_INTERVAL_SECONDS="20"
cargo run
```

You can alternatively copy `.env.example` to `.env` and edit it locally. Do not commit `.env` files.

## Find the Mac mini LAN IP

On the Mac mini, open **System Settings → Network**, select the active Ethernet or Wi-Fi connection, and read its IP address. From Terminal you can also try:

```bash
ipconfig getifaddr en0
```

Ethernet may use another interface; `ifconfig` lists all interfaces. Use the private LAN address, commonly beginning with `192.168.` or `10.`. Keep the Node server bound to `0.0.0.0`, allow incoming port 4000 through the macOS firewall if prompted, and do not expose the port to the public internet.

## Stable device identity

The first run generates a UUID in the operating system's standard per-user application configuration directory. Future runs reuse it. If the file is unreadable or contains an invalid UUID, the agent exits with a clear error instead of silently changing identity.

## Build and validate

```bash
cargo fmt --check
cargo check
cargo build
cargo build --release
```

The release executable is written to `target/release/pie22-remote-agent` on macOS/Linux or `target\\release\\pie22-remote-agent.exe` on Windows. Cargo package names cannot begin with a digit, so the binary uses `pie22` while the product remains 22Pie Remote Agent.

Set `RUST_LOG=debug` to display successful heartbeat acknowledgements. Normal info logging stays concise.
