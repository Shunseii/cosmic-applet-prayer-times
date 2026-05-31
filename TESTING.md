# Testing the applet

## 0. One-time: tooling on PATH

`cargo`/`just` live in `~/.cargo/bin`, which is already on your `PATH` for **new**
shells. In your current shell either open a new terminal or run `rehash` (zsh).
Verify:

```sh
cargo --version
just --version
```

## Option A — Quick visual test (floating window, fastest iteration)

```sh
cd ~/Projects/cosmic-applet-prayer-times
just run          # or: cargo run
```

- A small applet button appears **near the center of the screen** (it is *not*
  docked into the panel — that is expected for a standalone run).
- Click it → the popup opens: location + date header, the current-prayer hero
  with a live `H:MM:SS` countdown, the five-prayer list (current = accent +
  dot, next = accent dot, passed/upcoming plain), and a footer.
- The countdown updates every second.
- Stop it with `Ctrl+C` in the terminal.
- The `error loading system dark theme … list_button` lines are a benign
  cosmic dev-run fallback, not a failure.

## Option B — Real panel applet (the representative test)

```sh
cd ~/Projects/cosmic-applet-prayer-times
just build-release
sudo ~/.cargo/bin/just install     # full path: sudo's PATH doesn't include ~/.cargo/bin
```

Then add it: **Settings → Desktop → Panel (and/or Dock) → Configure applets →
add "Prayer Times"**. If it isn't listed, log out/in or restart the panel so
the new `.desktop` is picked up.

It now sits in the top panel as `<next prayer> <countdown>` (e.g. `Asr 1:23`);
click to open the popup. Remove later with:

```sh
sudo ~/.cargo/bin/just uninstall
```

## Adhan audio

No sound is bundled (licensing). Provide a file at either:

- `assets/adhan.ogg` (when running from the repo), or
- `~/.local/share/cosmic-applet-prayer-times/adhan.ogg`

Accepted: `.ogg`, `.mp3`, `.wav`, `.flac`. A CC0 recording to use:
<https://freesound.org/people/sonically_sound/sounds/639494/> — then:

```sh
ffmpeg -i your-file.flac -c:a libvorbis assets/adhan.ogg
```

At each prayer time (for prayers with the adhan enabled — all are, by default)
the file plays, the popup footer switches to **Stop / Snooze 5m**, and the panel
label shows `<prayer> adhan`. With no file present, the crossing is still
logged and the UI shows the playing state — just silent.

## Triggering the adhan on demand (without waiting for a real prayer time)

There is no settings UI yet, so the quickest ways to see/hear it fire now:

1. **Shift the system clock** to ~1 minute before a prayer time, run `just run`,
   and watch it cross. (Reset your clock afterwards.)
2. **Edit `src/config.rs`** — change the default `latitude`/`longitude` to a
   location whose next prayer is imminent — then `cargo run`.

Run from a terminal to watch the logs; at the crossing you'll see
`playing adhan` (or `adhan time (no audio file configured)`).

## Known MVP limitations (by design / deferred)

- Times use your machine's local timezone with **hardcoded Toronto coordinates**
  (ISNA method) — correct only if your system timezone matches (America/Toronto).
  Real location/timezone handling comes with the settings milestone.
- **Hijri date** is not shown (the `salah` crate doesn't provide it).
- No settings UI and no language/RTL switch yet (intentionally out of MVP scope).
