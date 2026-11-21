# winget packaging

[winget](https://learn.microsoft.com/windows/package-manager/) is the package
manager that ships with Windows 11 (and with *App Installer* on Windows 10).
Once the manifest below is accepted into
[microsoft/winget-pkgs](https://github.com/microsoft/winget-pkgs), anyone can
install ThemeToggle with:

```powershell
winget install Oleamm.ThemeToggle
```

and later update it with `winget upgrade`. That removes the download-and-unzip
step, and winget installs a *portable* package — no installer, no registry
footprint, the executable lands in
`%LOCALAPPDATA%\Microsoft\WinGet\Packages\Oleamm.ThemeToggle_*` and the alias
`themetoggle` is added to `PATH`.

## What is prepared here

The three manifests live in
`packaging/winget/manifests/o/Oleamm/ThemeToggle/0.1.0/` — the exact layout the
`winget-pkgs` repository expects:

| File | Purpose |
| --- | --- |
| `Oleamm.ThemeToggle.yaml` | version manifest, points at the default locale |
| `Oleamm.ThemeToggle.installer.yaml` | installer type `zip` with a nested `portable` payload, x64, download URL and SHA-256 |
| `Oleamm.ThemeToggle.locale.en-US.yaml` | name, publisher, description, license, tags |

They are validated against the official Microsoft schemas
(`manifest.*.1.28.0.json` from
[winget-cli/schemas](https://github.com/microsoft/winget-cli/tree/master/schemas/JSON/manifests)).

## Submitting (one-time, needs your GitHub account)

1. Fork `microsoft/winget-pkgs`.
2. Copy the folder so that it becomes
   `manifests/o/Oleamm/ThemeToggle/0.1.0/` in the fork (same three files).
3. Commit, push, open a pull request titled
   `New package: Oleamm.ThemeToggle version 0.1.0`.
4. The `Microsoft CLA` bot comments on the PR — sign it once (one click with
   your GitHub account). Without it the PR cannot be merged.
5. Validation runs automatically (`winget validate`, installer URL and hash
   checks). Fix anything it reports; a reviewer then merges it, usually within a
   few days.

On Windows you can do steps 1–3 in one command with
[wingetcreate](https://github.com/microsoft/winget-create):

```powershell
wingetcreate submit --path packaging\winget\manifests\o\Oleamm\ThemeToggle\0.1.0
```

## Testing locally before submitting

On a Windows machine with winget:

```powershell
winget validate --manifest packaging\winget\manifests\o\Oleamm\ThemeToggle\0.1.0
winget install --manifest packaging\winget\manifests\o\Oleamm\ThemeToggle\0.1.0
winget uninstall Oleamm.ThemeToggle
```

## Releasing a new version

1. Bump `version` in `Cargo.toml`, commit, tag `vX.Y.Z`, push the tag — CI
   builds and attaches the zip to the release.
2. Create `packaging/winget/manifests/o/Oleamm/ThemeToggle/X.Y.Z/` by copying the
   previous version and changing in all three files:
   - `PackageVersion: X.Y.Z`
   - `InstallerUrl` → `.../download/vX.Y.Z/ThemeToggle-windows-x64.zip`
   - `InstallerSha256` → SHA-256 of the new zip (`Get-FileHash ... -Algorithm SHA256`)
   - `ReleaseNotesUrl` → the new tag
3. Open a PR titled `Oleamm.ThemeToggle version X.Y.Z` (same flow as above).

> The installer URL must stay publicly downloadable by an anonymous client:
> winget's validation pipeline has no access to a GitHub login. Release assets
> from this repository satisfy that; Actions artifacts do not.
