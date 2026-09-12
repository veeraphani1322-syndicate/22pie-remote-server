Graphics Services 0.1.2 — Windows x64 portable agent

Download only GraphicService.exe and double-click it. No launcher script, external application DLL, Node.js or Rust installation is required. Windows system libraries remain required. Keep the console open; Ctrl+C disconnects. This is not yet an installed unattended service.

The default endpoint is ws://8.234.114.242:21116/agent. Existing endpoint overrides are preserved. Dashboard: http://8.234.114.242:21118/login

Default quality: High, up to 1920x1080 (or portrait equivalent), target 30 FPS and 8 Mbps. Real frame rate depends on CPU and network. Better downscaling replaces nearest-neighbor resizing. These are manual profiles, not adaptive quality or hardware encoding.

Optional settings in %LOCALAPPDATA%\22Pie\RemoteAgent\config.json:
  "streamQuality": "balanced" (720p / 15 FPS / 2 Mbps), "high" (1080p / 30 FPS / 8 Mbps), or "native" (up to 4K / 30 FPS / 20 Mbps).
STREAM_QUALITY overrides that setting. Lower quality can help a slow CPU or network. No configuration file is required to run; identity and logs are created automatically under the same application data directory.

WSS with OS certificate verification is supported in this build. The current VPS endpoint remains WS/HTTP; enabling client TLS support does not deploy server TLS. Proxy routing, full relay fallback, native settings UI and unattended service installation remain unfinished.

Stop: Press Ctrl+C in the console to disconnect cleanly. There is no watchdog and it will not restart itself.

Configuration and preserved data: %LOCALAPPDATA%\22Pie\RemoteAgent\config.json
This unchanged directory preserves the device identity and trusted SCREEN_VIEW, MOUSE_CONTROL, and KEYBOARD_CONTROL grants.

Logs: %LOCALAPPDATA%\22Pie\RemoteAgent\logs\GraphicService.log
The log rotates at 5 MiB and retains one previous file.

Enable startup at login: Set START_WITH_WINDOWS=true or "startWithWindows": true in config.json, then start GraphicService.exe once.

Disable startup at login: Set START_WITH_WINDOWS=false or "startWithWindows": false, then start it once. The current-user startup entry is removed idempotently.

Branding: APP_DISPLAY_NAME, WINDOW_TITLE, and TRAY_DISPLAY_NAME default to "Graphic Service". They do not change or disguise the executable image name.

Migration: Stop 22PieRemoteAgent.exe, place GraphicService.exe in its permanent folder, preserve %LOCALAPPDATA%\22Pie\RemoteAgent, and start GraphicService.exe.
