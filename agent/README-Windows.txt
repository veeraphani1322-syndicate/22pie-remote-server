Graphic Service — authorized remote-access agent

Start: Double-click GraphicService.exe. Keep the visible console open while using remote access. Startup at login is disabled by default.

Stop: Press Ctrl+C in the console to disconnect cleanly. There is no watchdog and it will not restart itself.

Configuration and preserved data: %LOCALAPPDATA%\22Pie\RemoteAgent\config.json
This unchanged directory preserves the device identity and trusted SCREEN_VIEW, MOUSE_CONTROL, and KEYBOARD_CONTROL grants.

Logs: %LOCALAPPDATA%\22Pie\RemoteAgent\logs\GraphicService.log
The log rotates at 5 MiB and retains one previous file.

Enable startup at login: Set START_WITH_WINDOWS=true or "startWithWindows": true in config.json, then start GraphicService.exe once.

Disable startup at login: Set START_WITH_WINDOWS=false or "startWithWindows": false, then start it once. The current-user startup entry is removed idempotently.

Branding: APP_DISPLAY_NAME, WINDOW_TITLE, and TRAY_DISPLAY_NAME default to "Graphic Service". They do not change or disguise the executable image name.

Migration: Stop 22PieRemoteAgent.exe, place GraphicService.exe in its permanent folder, preserve %LOCALAPPDATA%\22Pie\RemoteAgent, and start GraphicService.exe.
