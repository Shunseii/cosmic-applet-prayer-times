# cosmic-applet-prayer-times — Design

A COSMIC panel applet that displays Islamic prayer times and plays the adhan
at prayer time. Target: Pop!_OS 24.04 (COSMIC desktop), Wayland.

---

## 1. Goals

- Panel button showing the next prayer + countdown (e.g. "Asr in 1:23").
- Popup listing all five daily prayers with their times, highlighting the next one.
- Play an adhan audio file at each prayer time.
- Per-prayer adhan toggle, volume, mute/snooze.
- Configurable calculation method, madhab, and location.

### Non-goals (v1)
- No Qibla compass UI (calc is free via `salah`, but defer the UI).
- No Hijri calendar widget.
- No multi-location / travel mode.

---

## 2. Stack

| Concern            | Choice                                                       |
| ------------------ | ------------------------------------------------------------ |
| GUI toolkit        | `libcosmic` (iced-based), `applet` feature                   |
| Async runtime      | `tokio`                                                      |
| Prayer time calc   | `salah` crate (Adhan / Meeus algorithms)                     |
| Audio playback     | `rodio`                                                      |
| Config persistence | `cosmic-config`                                              |
| Display server     | Wayland                                                      |
| Scaffold           | `cargo generate gh:pop-os/cosmic-applet-template`            |

---

## 3. Architecture

Elm architecture (libcosmic `Application` trait): init / update / view / subscription.

```
                ┌─────────────────────────────────────────┐
                │  Applet process (cosmic::Application)     │
                │                                           │
   panel  ◄─────┤  view()      → panel button + popup       │
                │  update(Msg) → state transitions          │
                │  subscription() ─┐                        │
                └──────────────────┼────────────────────────┘
                                   │
            ┌──────────────────────┼───────────────────────┐
            │ tick (every 1s/30s)  │  config-changed        │
            ▼                      ▼                        ▼
   recompute "next prayer"   reload settings        play adhan when
   + countdown               (method/madhab/loc)    now crosses a prayer time
                                                     └─► rodio sink
```

### Open architectural decision: who owns playback?
The applet process can't wake a suspended machine, so a prayer can be missed
if the laptop is asleep at adhan time.

- **Option A (v1):** playback lives in the applet's subscription loop. Simple,
  but misses prayers across sleep/wake and only fires while the applet runs.
- **Option B (later):** a separate systemd **user** service owns scheduled
  playback; the applet is UI + config only. More reliable; survives applet
  restarts. Requires writing/refreshing systemd timer units from config.

Decision: ship A, design the playback module behind a trait so B can be added
without touching the UI.

---

## 4. State

```
struct AppModel {
    config: Config,             // persisted via cosmic-config
    times: PrayerTimes,         // today's five times, recomputed at midnight
    next: (Prayer, DateTime),   // derived each tick
    now: DateTime,
    playing: Option<Sink>,      // active adhan, for stop/snooze
    last_fired: Option<Prayer>, // de-dupe guard so adhan plays once
    popup: Option<Id>,          // open popup window id
}

struct Config {
    latitude: f64,
    longitude: f64,
    method: CalculationMethod,  // MWL, ISNA, Egypt, Makkah, Karachi, ...
    madhab: Madhab,             // Hanafi | Shafi
    adhan_enabled: [bool; 5],   // per-prayer
    volume: f32,
    adhan_path: Option<PathBuf>,// custom file, else bundled default
    time_format: TimeFormat,    // 12h | 24h
}
```

---

## 5. Messages

```
enum Message {
    Tick,                       // timer subscription
    TogglePopup,
    ConfigChanged(Config),
    PlayAdhan(Prayer),          // fired when now crosses a prayer time
    StopAdhan,                  // snooze / mute current
    SetMethod, SetMadhab, SetLocation, SetVolume, ToggleAdhan(Prayer), // settings
}
```

### Playback trigger logic (in `update(Tick)`)
1. recompute `now`; if past midnight, recompute `times` for the new day.
2. find `next` prayer + countdown for the panel label.
3. for each prayer whose time <= now and `last_fired != prayer` and
   `adhan_enabled[prayer]`: dispatch `PlayAdhan`, set `last_fired`.

---

## 6. UI

### Panel button
- Icon (mosque/minaret) + compact text: next prayer name + countdown.
- COSMIC themes the button automatically; use design tokens, no hardcoded sizes.

### Popup
- Five rows: prayer name | time, next prayer highlighted.
- Footer: settings access, snooze/stop button while adhan is playing.

### Settings
- Location (lat/long input; v1 manual, later GeoClue auto-detect).
- Calculation method dropdown.
- Madhab toggle.
- Per-prayer adhan switches.
- Volume slider, time format toggle, custom adhan file picker.

---

## 7. Location strategy
- **v1:** manual lat/long (or a city preset list) in settings.
- **Later:** GeoClue2 over D-Bus, or IP geolocation fallback.

---

## 8. Packaging
- `justfile` from the template (`just build-release`, `just install`).
- `.desktop` + applet metadata so it registers in COSMIC panel config.
- Bundle a default adhan audio asset (check licensing — ship a CC0/permissive
  recording or prompt the user to supply their own).

---

## 9. Risks / unknowns
1. iced/Elm learning curve — the real time cost, not the prayer math.
2. libcosmic is pre-1.0 and churns; pin versions, expect breakage.
3. Missed adhan across suspend → drives the Option B systemd path eventually.
4. Adhan audio licensing for redistribution.

---

## 10. Milestones
- [ ] M0: scaffold from template, empty panel button + popup renders.
- [ ] M1: `salah` wired, popup shows today's five times (hardcoded location).
- [ ] M2: panel label shows next prayer + live countdown (timer subscription).
- [ ] M3: `rodio` plays bundled adhan at prayer time, with de-dupe guard.
- [ ] M4: settings UI + `cosmic-config` persistence (method/madhab/location).
- [ ] M5: per-prayer toggles, volume, snooze/stop, custom adhan file.
- [ ] M6: packaging + panel registration.
- [ ] Later: GeoClue auto-location; systemd user service for reliable playback.
