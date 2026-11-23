# genubar-installer

Self-contained installer (`genubar-installer`) offering both a GTK4/Libadwaita GUI and headless CLI flags to install, update, and cleanly uninstall Genubar components and the GNOME Shell extension.

## What It Installs

1. **Binaries (`~/.local/bin/`):** `genubar-daemon`, `genubar-cli`, `genubar-settings`.
2. **Desktop Entry & Icons (`~/.local/share/applications/`, `~/.local/share/icons/hicolor/scalable/apps/`):** `org.genubar.Settings.desktop` plus `org.genubar.Settings.svg` / `org.genubar.Installer.svg` so **Genubar Settings** appears with its application icon in GNOME Overview search.
3. **GTK4 Hook Library (`~/.local/lib/genubar/`):** `libgenubar_gtk4_hook.so` and `active_gtk4_hook.so`.
4. **GNOME Shell Extension (`~/.local/share/gnome-shell/extensions/genubar@github.com/`):** Deploys the bundled extension files (`genubar-platform::EMBEDDED_EXT_FILES`), cleans up stale module files if present, and runs `gnome-extensions enable genubar@github.com`.
5. **Systemd User Unit (`~/.config/systemd/user/genubar.service`):** Configures `Environment="PATH=%h/.local/bin:..."`, `PartOf=graphical-session.target`, and `WantedBy=graphical-session.target` so `genubar-daemon` starts automatically on login and restarts cleanly across Wayland logout/login cycles, then runs `systemctl --user daemon-reload` and `systemctl --user enable --now genubar.service`.

## Artifact Resolution (artifacts.rs)

When installing (`perform_install_or_update`), `genubar-installer` searches local paths (`$GENUBAR_TARGET_DIR`, workspace `target/{release,debug}`, executable directory, or `~/.cache/genubar/installer/v<ver>/`) first; if binaries are not found locally, it falls back to downloading `genubar-v<ver>-linux-<arch>.tar.gz` from GitHub Releases into the versioned cache directory.

## CLI Usage

```bash
# Launch graphical installer UI
genubar-installer

# Headless install or update
genubar-installer --install
genubar-installer --update

# Install/sync only the embedded GNOME Shell extension
genubar-installer --install-extension

# Uninstall binaries, hook, extension, and systemd unit (keeps ~/.config/genubar)
genubar-installer --uninstall

# Full uninstall including configs and caches
genubar-installer --uninstall --purge
```

## Testing

```bash
cargo test -p genubar-installer
```
