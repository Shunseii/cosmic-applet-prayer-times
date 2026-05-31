# cosmic-applet-prayer-times

A COSMIC panel applet that shows the five daily Islamic prayer times,
highlights the current prayer with a live countdown to the next, and plays the
adhan at prayer time.

Panel button + popup + adhan playback + a **settings page** (location,
calculation method, madhab, per-prayer adhan toggles, volume, time format,
custom adhan file), persisted via `cosmic-config`. Defaults (in
`src/config.rs`) are Toronto, Canada with the ISNA method. See `DESIGN.md` for
the full product spec and `design.pen` for the visual design.

Not yet implemented: language switch / Arabic + RTL, and the displayed location
label is still static text (the lat/long are editable, but the "Toronto, Canada"
caption doesn't yet reflect custom coordinates).

## Build dependencies

Rust (stable) plus a few system libraries:

```sh
sudo apt install -y build-essential pkg-config \
    libxkbcommon-dev libwayland-dev libasound2-dev libfontconfig-dev
```

(`libasound2-dev` is for adhan audio via `rodio`; the rest are for libcosmic.)

## Build & run

```sh
make build              # or: cargo build --release
make run                # run for testing (or: just run)
```

## Install (registers the applet with COSMIC)

```sh
cd ~/Projects/cosmic-applet-prayer-times
make build && sudo make install     # or: sudo ~/.cargo/bin/just install
```

`make install` copies the already-built release binary, so build as your user
first (it won't recompile as root). Defaults to `PREFIX=/usr`; override with
`sudo make install PREFIX=/usr/local`.

Then add it via **Settings → Desktop → Panel → Configure panel applets** (log
out/in or restart the panel if it doesn't appear immediately).

Uninstall with `sudo make uninstall`.

## Adhan audio

No audio file is bundled (licensing). Drop a file named `adhan.ogg` (or `.mp3`,
`.wav`, `.flac`) at one of:

- `assets/adhan.ogg` (when running from the repo), or
- `~/.local/share/cosmic-applet-prayer-times/adhan.ogg`

If no file is found, prayer crossings are logged and the UI still reflects the
"playing" state, but no sound plays.

## Source layout

| File | Responsibility |
|------|----------------|
| `src/main.rs`   | Entry point; starts the applet event loop. |
| `src/app.rs`    | The `cosmic::Application`: panel button, popup view, tick/adhan logic. |
| `src/prayer.rs` | `salah` wrapper: schedule, current/next, countdown, adhan crossings. |
| `src/audio.rs`  | Adhan playback on a dedicated thread (rodio), behind a command channel. |
| `src/config.rs` | Hardcoded config defaults (shape mirrors the future settings model). |

## Status vs. milestones (`DESIGN.md`)

- [x] M0–M3: panel button, prayer list, live countdown, adhan playback with de-dupe + stop/snooze.
- [x] M4–M5: settings UI (method/madhab/location/per-prayer toggles/volume/time format/adhan file) + `cosmic-config` persistence.
- [ ] M6: packaging polish + panel registration metadata.
- [ ] Later: GeoClue auto-location; systemd user service for reliable playback; i18n / Arabic + RTL.
