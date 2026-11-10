# ThemeToggle

A single-icon Windows tray app that flips the system between the light and the
dark appearance on one click, so the Settings app is never needed for it.

- **Left click** the tray icon → light ⇄ dark.
- **Right click** → small menu: switch theme, start with Windows, quit.
- The icon always shows the current appearance (sun = light, moon = dark) and
  is coloured for the taskbar it is drawn on, so it never disappears into it.
- No admin rights, no service, no background polling of the shell: one process,
  one thread, one message loop.

## Why

Changing the Windows theme by hand takes Settings → Personalization → Colors →
“Choose your mode”. This app puts that switch one click away, permanently
visible next to the clock.

## How it works

Windows stores the per-user appearance in the registry:

| Value | Drives |
| --- | --- |
| `HKCU\Software\Microsoft\Windows\CurrentVersion\Themes\Personalize\AppsUseLightTheme` | application chrome |
| `HKCU\Software\Microsoft\Windows\CurrentVersion\Themes\Personalize\SystemUsesLightTheme` | taskbar, Start, Action Center |

`0` is dark, `1` is light. Writing those values alone does not repaint anything:
running programs only re-read them when they receive `WM_SETTINGCHANGE` with
`lParam = "ImmersiveColorSet"` — the exact broadcast the Settings app sends. So
the app writes both values and broadcasts that message, which is why open apps
switch immediately instead of after a sign-out.

The same values are read back to draw the icon, and a 2-second thread timer
keeps the icon in sync if the theme is changed somewhere else.

## Install

1. Download `ThemeToggle-windows-x64.zip` from the latest successful
   [Actions run](/actions/workflows/windows-release.yml) and unpack
   `ThemeToggle.exe` anywhere you like.
2. Run it. The icon appears in the notification area.
3. Optional: enable **Start with Windows** in the right-click menu.

> Windows hides new tray icons in the “hidden icons” flyout by default. Drag the
> icon onto the taskbar (or enable it in Settings → Personalization → Taskbar →
> Other system tray icons) to keep it visible.

### Uninstall

Quit from the menu, turn **Start with Windows** off first if it was on, then
delete the executable. The app writes nothing else except its log file
(`%LOCALAPPDATA%\ThemeToggle\logs\themetoggle.log`) and the registry values
listed above — which are the same values the Settings app itself owns, so they
are left at whatever theme you had last.

## Build

Requirements: Rust stable, MSVC build tools (the usual Windows Rust setup).

```bat
build.bat                 :: debug build
build_release.bat         :: release build staged as dist\ThemeToggle.exe
run_tests.bat             :: cargo test --all-targets
```

Or plainly:

```
cargo build --release
cargo test --all-targets
```

`build.rs` embeds `assets/themetoggle.ico` and `app.manifest` into the binary and
bakes a build id (`version+commit.timestamp`) into it; the id is written to the
log on every start, so a shipped .exe can be traced back to a commit.

## CI

| Workflow | Trigger | What it does |
| --- | --- | --- |
| `windows-release.yml` | push to `main`, manual | tests, release build, uploads `ThemeToggle-windows-x64.zip` + `ThemeToggle.exe` |
| `ci-check.yml` | push to `fix/**`, pull requests, manual | tests, release build |

## Behaviour details

- **Single instance.** A named mutex keeps a second copy from adding a second
  icon that would fight the first one over the same registry values.
- **Double clicks.** A double click sends two button-up events; clicks closer
  than 400 ms are ignored so the theme does not flip twice.
- **Logging.** `%LOCALAPPDATA%\ThemeToggle\logs\themetoggle.log`, rotated at
  1 MiB. Set `THEMETOGGLE_LOG=debug` (or `trace`, `warn`, `off`) for more or
  less detail.
- **Icons** are generated, not borrowed: `tools/make_icons.py` renders the sun,
  the moon and the application icon analytically (4×4 supersampling, no imaging
  library) and writes both PNGs plus the multi-size `.ico`.

## Repository layout

```
src/main.rs        entry point, fatal-error box
src/tray.rs        tray icon, context menu, Win32 message pump
src/theme.rs       registry read/write + WM_SETTINGCHANGE broadcast
src/icons.rs       embedded PNG glyphs, RGBA decode
src/autostart.rs   per-user Run key handling
src/instance.rs    single-instance mutex
src/logfile.rs     file logger with rotation
```

## License

MIT — see [LICENSE](LICENSE).
