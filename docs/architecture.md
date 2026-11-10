# Architecture

## Process shape

One process, one thread, one Win32 message loop. There is no async runtime, no
worker thread and no window: `tray-icon` and `muda` create the hidden windows
that own the tray icon and the popup menu, and this crate pumps their messages.

```
main()
 ├─ logfile::init()                       %LOCALAPPDATA%\ThemeToggle\logs
 ├─ SingleInstanceGuard::acquire()        named mutex, silent exit if taken
 └─ tray::run()
     ├─ Tray::new()                       menu + TrayIcon (menu_on_left_click = false)
     ├─ TrayIconEvent::set_event_handler  left button up  -> PostThreadMessage(WM_TOGGLE)
     ├─ MenuEvent::set_event_handler      menu item      -> PostThreadMessage(WM_*)
     ├─ SetTimer(2 s)                     WM_TIMER       -> re-read theme, repaint icon
     └─ GetMessageW / DispatchMessageW loop
         ├─ WM_TOGGLE          theme::toggle()  + render()
         ├─ WM_TOGGLE_AUTOSTART autostart::set_enabled()
         ├─ WM_QUIT_REQUESTED   break
         └─ everything else     DispatchMessageW (tray + menu windows)
```

## Why the callbacks only post a message

Both event handlers run inside another window's procedure on the pump thread.
The toggle itself opens a registry key and broadcasts `WM_SETTINGCHANGE` to
`HWND_BROADCAST` with a timeout of up to 2 s. Doing that inside a window
procedure would block that procedure and, because the broadcast also reaches our
own windows, re-enter it. Posting a thread message keeps the heavy work in the
loop body and makes the ordering predictable.

## Why both registry values

`AppsUseLightTheme` alone leaves the taskbar light while apps go dark (the
“Custom” personalization mode). The app always writes both, so one click yields
a consistent theme. The icon contrast, however, follows `SystemUsesLightTheme`,
because the tray sits on the taskbar and not on application chrome.

## Failure behaviour

| Situation | What happens |
| --- | --- |
| Registry key cannot be opened or written | logged at `error`, icon and theme stay as they were |
| Broadcast reaches no window | logged at `warn`; values are still written, so the theme applies on next sign-in |
| Tray icon cannot be created | fatal: message box with the error, exit code 1 |
| Second instance | logged at `info`, exit code 0, no dialog |
| Autostart write fails | logged at `error`, checkbox keeps its real state on next start |

## Deliberate non-goals

- No scheduled sunset/sunrise switching (that is Auto Dark Mode's job).
- No accent colour, transparency or wallpaper handling — only the light/dark
  mode pair, which is what the tray click means.
- No global hotkey: the icon is the interface.
