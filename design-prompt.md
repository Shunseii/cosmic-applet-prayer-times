# Design brief — COSMIC prayer-times applet (Pencil)

You are designing the UI for a **COSMIC desktop panel applet** (Pop!_OS 24.04,
Wayland) that shows Islamic prayer times and plays the adhan. Produce the visual
design in a Pencil `.pen` file at `design.pen` in this repo.

A written product spec already exists at `DESIGN.md` — read it first for the full
feature set, state model, and milestones. This brief covers only the *visual /
interaction* design you need to produce.

## Before you start
1. Call `get_editor_state(include_schema: true)` to load the current `.pen`
   schema — required before any other Pencil tool.
2. Call `get_guidelines` and follow Pencil's design conventions.
3. Create the design at `design.pen` (the spec lives separately in `DESIGN.md`;
   do not overwrite it).

## What this is (constraints that shape the design)
- It's a **panel applet**, not a full app. Two surfaces matter most:
  1. A tiny **panel button** that lives in the COSMIC top panel.
  2. A small **popup window** that opens when the button is clicked
     (think ~360px wide, content-height — like a dropdown menu, not a window).
- It must look **native to COSMIC**: rounded corners, system accent color,
  generous spacing, the COSMIC panel/popup aesthetic. Design for both
  **light and dark** themes.
- Prayer names appear in Arabic and English — account for **RTL** Arabic text
  sitting alongside LTR times.
- Use **design tokens / a consistent scale**, not arbitrary one-off values.

## Screens / frames to design

### 1. Panel button (a few variants)
- Compact: small mosque/minaret icon + next prayer + countdown, e.g. "Asr 1:23".
- An "adhan playing now" state (subtle pulse/highlight).
- Icon-only fallback (narrow panel).

### 2. Popup — main view
- Header: today's date (Gregorian + Hijri), current location.
- Five prayer rows: Fajr, Dhuhr, Asr, Maghrib, Isha — each shows
  Arabic name, English name, and time. **Highlight the next prayer** and show a
  countdown to it.
- A row treatment for "already passed today" vs "upcoming" vs "next".
- Footer: settings (gear) entry point; when adhan is playing, a
  **Stop / Snooze** control appears.

### 3. Popup — settings view
- Location (manual lat/long or city; note auto-detect is a later phase).
- Calculation method dropdown (MWL, ISNA, Egypt, Makkah, Karachi, Tehran…).
- Madhab toggle (Hanafi / Shafi).
- Per-prayer adhan switches (5 toggles).
- Volume slider; time format (12h/24h) toggle; custom adhan file picker.

### 4. Reusable components
- Prayer row (default / next-highlighted / passed states).
- Toggle switch, dropdown, slider, primary/secondary buttons — matching COSMIC.

## Visual direction
- Calm, focused, a touch reverent — not flashy. Prayer time is the hero.
- Clear visual hierarchy: next prayer + countdown is the single most prominent
  element in the popup.
- Strong contrast and large enough tap/click targets; legible Arabic typography.

## Deliverable
A `design.pen` containing the panel-button variants, the popup main view, the
settings view, light + dark versions, and the reusable component set — laid out
so a developer can implement the libcosmic UI directly from it. When done,
summarize the frames you created and any open design questions.
