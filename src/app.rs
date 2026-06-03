// SPDX-License-Identifier: GPL-3.0-only

use std::path::PathBuf;
use std::sync::LazyLock;
use std::time::Duration as StdDuration;

use cosmic::app::{Core, Task};
use cosmic::cosmic_config::{self, CosmicConfigEntry};
use cosmic::iced::platform_specific::shell::wayland::commands::popup::{destroy_popup, get_popup};
use cosmic::iced::widget::{column, row};
use cosmic::iced::{window::Id, Alignment, Length, Limits, Subscription};
use cosmic::cosmic_theme::Spacing;
use cosmic::widget::{button, container, divider, dropdown, icon, slider, space, text, text_input, toggler};
use cosmic::{theme, Element};

use salah::prelude::{DateTime, Duration, Utc};

use crate::audio::AudioHandle;
use crate::config::{
    ActiveAdhan, CalcMethod, Config, Language, MadhabPref, PlaybackState, TimeFormat,
};
use crate::i18n;
use crate::prayer::{self, RowState, Schedule, Slot};

static AUTOSIZE_ID: LazyLock<cosmic::widget::Id> =
    LazyLock::new(|| cosmic::widget::Id::new("ppt-panel-autosize"));

const SNOOZE: Duration = Duration::minutes(5);

/// `cosmic-config` id for the shared, cross-instance adhan playback state.
const PLAYBACK_ID: &str = "io.github.shunseii.CosmicAppletPrayerTimes.State";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Page {
    Main,
    Settings,
}

pub struct PrayerApplet {
    core: Core,
    popup: Option<Id>,
    page: Page,
    config: Config,
    config_handle: Option<cosmic_config::Config>,
    /// Shared, cross-instance adhan playback state and its backing handle.
    playback: PlaybackState,
    playback_handle: Option<cosmic_config::Config>,
    /// This process's PID, used to claim ownership of audio output.
    owner: u32,
    /// The slot this process actually started audio for, so we don't restart it
    /// on every playback-state update. `None` when this process isn't the owner.
    audio_started_for: Option<u8>,
    audio: AudioHandle,
    schedule: Option<Schedule>,
    now: DateTime<Utc>,
    /// Instant of the previous tick; prayers crossing `(last_tick, now]` fire once.
    last_tick: DateTime<Utc>,
    /// The prayer whose adhan is currently sounding, if any.
    playing: Option<Slot>,
    /// Pending snooze: replay `slot`'s adhan at the given instant.
    snooze: Option<(DateTime<Utc>, Slot)>,
    /// True while a manual "test adhan" is playing from the settings page.
    testing: bool,
    // Editable text buffers for the settings coordinate inputs.
    lat_text: String,
    lon_text: String,
}

#[derive(Debug, Clone)]
pub enum Message {
    TogglePopup,
    PopupClosed(Id),
    Tick,
    StopAdhan,
    Snooze,
    TestAdhan,
    OpenSettings,
    CloseSettings,
    SetLatitude(String),
    SetLongitude(String),
    SetLocationName(String),
    SetMethod(usize),
    SetMadhab(usize),
    ToggleAdhan(usize, bool),
    SetVolume(f32),
    SetTimeFormat(usize),
    SetLanguage(usize),
    PickAdhanFile,
    AdhanFilePicked(Option<PathBuf>),
    /// The shared `Config` changed on disk (e.g. settings edited on another
    /// monitor's applet instance).
    ConfigUpdated(Config),
    /// The shared adhan playback state changed (another instance started or
    /// stopped the adhan).
    PlaybackUpdated(PlaybackState),
}

impl PrayerApplet {
    fn ensure_schedule(&mut self) {
        let today = prayer::today_local();
        let stale = self
            .schedule
            .as_ref()
            .map(|s| s.date() != today)
            .unwrap_or(true);
        if stale {
            self.recompute();
        }
    }

    fn recompute(&mut self) {
        let today = prayer::today_local();
        match Schedule::compute(&self.config, today) {
            Ok(s) => self.schedule = Some(s),
            Err(err) => tracing::error!(%err, "failed to compute prayer times"),
        }
    }

    fn persist(&self) {
        if let Some(handle) = &self.config_handle {
            if let Err(err) = self.config.write_entry(handle) {
                tracing::error!(?err, "failed to persist config");
            }
        }
    }

    fn persist_playback(&self) {
        if let Some(handle) = &self.playback_handle {
            if let Err(err) = self.playback.write_entry(handle) {
                tracing::error!(?err, "failed to persist playback state");
            }
        }
    }

    /// Claim ownership and announce a new adhan to all instances. The audio is
    /// not started here; `apply_playback` does that once the shared state is
    /// reconciled, so exactly one process (the last writer) ends up the owner.
    fn start_play(&mut self, slot: Slot) {
        self.playback.active = Some(ActiveAdhan {
            slot: slot.index() as u8,
            owner: self.owner,
        });
        self.persist_playback();
        self.apply_playback();
    }

    /// Clear the shared playback state and stop audio everywhere.
    fn stop_play(&mut self) {
        self.playback.active = None;
        self.persist_playback();
        self.apply_playback();
    }

    /// Reconcile local audio + UI with the shared playback state. Called both
    /// after a local change and when another instance updates the state.
    fn apply_playback(&mut self) {
        match self.playback.active {
            Some(ActiveAdhan { slot, owner }) => {
                self.playing = Some(Slot::from_index(slot as usize));
                if owner == self.owner {
                    // We own this adhan: start audio once.
                    if self.audio_started_for != Some(slot) {
                        if let Some(path) = self.config.resolved_adhan_path() {
                            self.audio.play(path, self.config.volume);
                        } else {
                            tracing::info!(slot, "adhan time (no audio file configured)");
                        }
                        self.audio_started_for = Some(slot);
                    }
                } else if self.audio_started_for.is_some() {
                    // Lost ownership (another instance won the claim): go quiet.
                    self.audio.stop();
                    self.audio_started_for = None;
                }
            }
            None => {
                self.playing = None;
                if self.audio_started_for.is_some() {
                    self.audio.stop();
                    self.audio_started_for = None;
                }
            }
        }
    }

    /// Compact panel label, e.g. "Asr 1h 23m" or "Maghrib adhan".
    fn panel_label(&self) -> String {
        let lang = self.config.language;
        if let Some(slot) = self.playing {
            return format!(
                "{} {}",
                slot.name_localized(lang),
                i18n::strings(lang).playing_label
            );
        }
        if self.testing {
            return i18n::strings(lang).playing_label.to_string();
        }
        let Some(schedule) = self.schedule.as_ref() else {
            return "Prayer times".to_string();
        };
        let status = schedule.status(self.now, lang);
        let s = i18n::strings(lang);
        let countdown = prayer::format_countdown(status.countdown, lang);
        // After sunrise the Fajr window has ended, so there's no current prayer —
        // show the next one ("Dhuhr in Xh XXm") instead of "Fajr ... left".
        if status.current_index == Some(Slot::Fajr.index()) && schedule.sunrise_passed(self.now) {
            format!("{} {} {}", status.next_label, s.connector_in, countdown)
        } else {
            format!("{} {} {}", status.current_label, countdown, s.left)
        }
    }

    fn main_view(&self) -> Element<'_, Message> {
        let spacing = theme::active().cosmic().spacing;
        let lang = self.config.language;
        let rtl = lang.is_rtl();
        let s = i18n::strings(lang);
        let align = if rtl { Alignment::End } else { Alignment::Start };

        let Some(schedule) = self.schedule.as_ref() else {
            return container(text::body(s.unavailable)).padding(16).into();
        };

        let status = schedule.status(self.now, lang);
        let pattern = self.config.time_format_pattern();

        let header = column![
            text::body(self.config.location_name.clone()),
            text::title4(prayer::hijri_date_string(lang)),
        ]
        .spacing(2)
        .width(Length::Fill)
        .align_x(align);

        let countdown_col = column![
            text::title2(prayer::format_countdown(status.countdown, lang))
                .class(cosmic::theme::Text::Accent),
            text::caption(s.time_left),
        ]
        .align_x(if rtl { Alignment::Start } else { Alignment::End });

        let next_group = pair(rtl, text::caption(s.next).into(), text::body(status.next_label.clone()).into(), spacing.space_xs);
        let next_time = text::body(i18n::localize_time(
            &schedule.next_time_string(&status, pattern),
            lang,
        ));

        let hero_inner = column![
            text::caption(s.current_prayer).class(cosmic::theme::Text::Accent),
            ends(rtl, text::title3(status.current_label.clone()).into(), countdown_col.into()),
            divider::horizontal::default(),
            ends(rtl, next_group, next_time.into()),
        ]
        .spacing(spacing.space_xs)
        .width(Length::Fill)
        .align_x(align);

        let hero = container(hero_inner)
            .padding(spacing.space_s)
            .width(Length::Fill)
            .class(cosmic::theme::Container::Card);

        // After sunrise (and before Dhuhr) the Fajr window has ended, so sunrise
        // is the "current" period instead of Fajr.
        let in_sunrise_window = status.current_index == Some(Slot::Fajr.index())
            && schedule.sunrise_passed(self.now);
        let mut list = column![].spacing(spacing.space_xxs);
        for slot in Slot::ALL {
            let mut state = schedule.row_state(slot, &status, self.now);
            if slot == Slot::Fajr && in_sunrise_window {
                state = RowState::Passed;
            }
            let time_str = i18n::localize_time(&schedule.local_time_string(slot, pattern), lang);
            list = list.push(prayer_row(
                rtl,
                slot.name_localized(lang),
                &time_str,
                state,
                spacing.space_xs,
            ));
            // Sunrise (informational) sits between Fajr and Dhuhr, and is
            // highlighted as current during the sunrise-to-Dhuhr window.
            if slot == Slot::Fajr {
                let sr_state = if in_sunrise_window {
                    RowState::Current
                } else if schedule.sunrise_passed(self.now) {
                    RowState::Passed
                } else {
                    RowState::Upcoming
                };
                let sr_time = i18n::localize_time(&schedule.sunrise_string(pattern), lang);
                list = list.push(prayer_row(rtl, s.sunrise, &sr_time, sr_state, spacing.space_xs));
            }
        }

        let gear = button::icon(icon::from_name("emblem-system-symbolic"))
            .on_press(Message::OpenSettings);

        let footer_right: Element<'_, Message> = if self.playing.is_some() {
            row![
                button::destructive(s.stop).on_press(Message::StopAdhan),
                button::standard(s.snooze).on_press(Message::Snooze),
            ]
            .spacing(spacing.space_xs)
            .into()
        } else {
            text::caption(format!(
                "{} · {}",
                self.config.method.label_localized(lang),
                self.config.madhab.label_localized(lang)
            ))
            .into()
        };
        let footer = ends(rtl, gear.into(), footer_right);

        column![
            header,
            hero,
            divider::horizontal::default(),
            list,
            divider::horizontal::default(),
            footer,
        ]
        .spacing(spacing.space_s)
        .padding(spacing.space_m)
        .into()
    }

    fn settings_view(&self) -> Element<'_, Message> {
        let spacing = theme::active().cosmic().spacing;
        let lang = self.config.language;
        let rtl = lang.is_rtl();
        let s = i18n::strings(lang);
        let g = spacing.space_s;

        let back_icon = if rtl { "go-next-symbolic" } else { "go-previous-symbolic" };
        let back = button::icon(icon::from_name(back_icon)).on_press(Message::CloseSettings);
        let title = text::title4(s.settings);
        let header: Element<'_, Message> = if rtl {
            row![space::horizontal().width(Length::Fill), title, back]
                .spacing(g)
                .align_y(Alignment::Center)
                .width(Length::Fill)
                .into()
        } else {
            row![back, title].spacing(g).align_y(Alignment::Center).into()
        };

        let method_opts: Vec<&str> = CalcMethod::ALL.iter().map(|m| m.label_localized(lang)).collect();
        let madhab_opts: Vec<&str> = MadhabPref::ALL.iter().map(|m| m.label_localized(lang)).collect();
        let time_opts: Vec<&str> = TimeFormat::ALL.iter().map(|t| t.label_localized(lang)).collect();
        let lang_opts: Vec<&str> = Language::ALL.iter().map(|l| l.label()).collect();

        let location = section(rtl, spacing, s.location, vec![
            item(rtl, g, s.location_name, text_input("", &self.config.location_name).on_input(Message::SetLocationName).into()),
            item(rtl, g, s.latitude, text_input("", &self.lat_text).on_input(Message::SetLatitude).into()),
            item(rtl, g, s.longitude, text_input("", &self.lon_text).on_input(Message::SetLongitude).into()),
        ]);

        let calculation = section(rtl, spacing, s.calculation, vec![
            item(rtl, g, s.method, dropdown(method_opts, Some(self.config.method.index()), Message::SetMethod).into()),
            item(rtl, g, s.madhab, dropdown(madhab_opts, Some(self.config.madhab.index()), Message::SetMadhab).into()),
        ]);

        let mut adhan_items: Vec<Element<'_, Message>> = Vec::new();
        for slot in Slot::ALL {
            let i = slot.index();
            let enabled = self.config.adhan_enabled[i];
            adhan_items.push(item(
                rtl,
                g,
                slot.name_localized(lang),
                toggler(enabled).on_toggle(move |b| Message::ToggleAdhan(i, b)).into(),
            ));
        }
        adhan_items.push(item(
            rtl,
            g,
            s.volume,
            slider(0.0..=1.0, self.config.volume, Message::SetVolume).step(0.05f32).into(),
        ));
        let adhan_name = self
            .config
            .adhan_path
            .as_ref()
            .and_then(|p| p.file_name())
            .map(|f| f.to_string_lossy().into_owned())
            .unwrap_or_else(|| s.default_file.to_string());
        let choose = button::icon(icon::from_name("folder-open-symbolic"))
            .on_press(Message::PickAdhanFile);
        adhan_items.push(item(rtl, g, s.adhan_file, choose.into()));
        // The filename gets its own wrapping line so a long name can't push the
        // picker icon off the edge of the popup.
        adhan_items.push(text::caption(adhan_name).width(Length::Fill).into());
        let test_button: Element<'_, Message> = if self.testing {
            button::destructive(s.stop).on_press(Message::StopAdhan).into()
        } else {
            button::standard(s.play_test).on_press(Message::TestAdhan).into()
        };
        adhan_items.push(item(rtl, g, s.test_adhan, test_button));
        if self.config.resolved_adhan_path().is_none() {
            adhan_items.push(text::caption(s.no_audio_file).into());
        }
        let adhan = section(rtl, spacing, s.adhan, adhan_items);

        let display = section(rtl, spacing, s.display, vec![
            item(rtl, g, s.time_format, dropdown(time_opts, Some(self.config.time_format.index()), Message::SetTimeFormat).into()),
            item(rtl, g, s.language, dropdown(lang_opts, Some(self.config.language.index()), Message::SetLanguage).into()),
        ]);

        column![header, location, calculation, adhan, display]
            .spacing(spacing.space_m)
            .padding(spacing.space_m)
            .into()
    }
}

impl cosmic::Application for PrayerApplet {
    type Executor = cosmic::executor::Default;
    type Flags = ();
    type Message = Message;

    const APP_ID: &'static str = "io.github.shunseii.CosmicAppletPrayerTimes";

    fn core(&self) -> &Core {
        &self.core
    }

    fn core_mut(&mut self) -> &mut Core {
        &mut self.core
    }

    fn init(core: Core, _flags: Self::Flags) -> (Self, Task<Self::Message>) {
        let config_handle = cosmic_config::Config::new(Self::APP_ID, Config::VERSION).ok();
        let config = config_handle
            .as_ref()
            .map(|h| Config::get_entry(h).unwrap_or_else(|(_errs, c)| c))
            .unwrap_or_default();

        let playback_handle =
            cosmic_config::Config::new(PLAYBACK_ID, PlaybackState::VERSION).ok();
        let playback = playback_handle
            .as_ref()
            .map(|h| PlaybackState::get_entry(h).unwrap_or_else(|(_errs, p)| p))
            .unwrap_or_default();

        let now = prayer::now_utc();
        let mut app = PrayerApplet {
            core,
            popup: None,
            page: Page::Main,
            lat_text: format!("{}", config.latitude),
            lon_text: format!("{}", config.longitude),
            config,
            config_handle,
            playback,
            playback_handle,
            owner: std::process::id(),
            audio_started_for: None,
            audio: AudioHandle::spawn(),
            schedule: None,
            now,
            last_tick: now,
            playing: None,
            snooze: None,
            testing: false,
        };
        app.recompute();
        app.apply_playback();
        (app, Task::none())
    }

    fn on_close_requested(&self, id: Id) -> Option<Message> {
        Some(Message::PopupClosed(id))
    }

    fn subscription(&self) -> Subscription<Self::Message> {
        let tick = cosmic::iced::time::every(StdDuration::from_secs(1)).map(|_| Message::Tick);

        // Watch the shared config + playback state so changes made by another
        // monitor's applet instance propagate here.
        let config = cosmic_config::config_subscription::<_, Config>(
            "config",
            Self::APP_ID.into(),
            Config::VERSION,
        )
        .map(|update| Message::ConfigUpdated(update.config));

        let playback = cosmic_config::config_subscription::<_, PlaybackState>(
            "playback",
            PLAYBACK_ID.into(),
            PlaybackState::VERSION,
        )
        .map(|update| Message::PlaybackUpdated(update.config));

        Subscription::batch([tick, config, playback])
    }

    fn update(&mut self, message: Self::Message) -> Task<Self::Message> {
        match message {
            Message::Tick => {
                self.now = prayer::now_utc();

                let rolled = self
                    .schedule
                    .as_ref()
                    .map(|s| s.date() != prayer::today_local())
                    .unwrap_or(true);
                if rolled {
                    self.ensure_schedule();
                }

                if let Some(schedule) = self.schedule.as_ref() {
                    let due: Vec<Slot> = schedule
                        .crossings(self.last_tick, self.now)
                        .into_iter()
                        .filter(|s| self.config.adhan_enabled[s.index()])
                        .collect();
                    if let Some(&slot) = due.last() {
                        self.start_play(slot);
                    }
                }

                if let Some((at, slot)) = self.snooze {
                    if self.now >= at {
                        self.snooze = None;
                        self.start_play(slot);
                    }
                }

                self.last_tick = self.now;
            }
            Message::StopAdhan => {
                self.testing = false;
                self.snooze = None;
                self.audio.stop();
                self.stop_play();
            }
            Message::Snooze => {
                let slot = self.playing;
                self.stop_play();
                if let Some(slot) = slot {
                    self.snooze = Some((self.now + SNOOZE, slot));
                }
            }
            Message::TestAdhan => {
                if let Some(path) = self.config.resolved_adhan_path() {
                    self.audio.play(path, self.config.volume);
                    self.testing = true;
                } else {
                    tracing::warn!("test adhan requested but no audio file is configured");
                }
            }
            Message::OpenSettings => self.page = Page::Settings,
            Message::CloseSettings => self.page = Page::Main,
            Message::SetLatitude(s) => {
                if let Ok(v) = s.trim().parse::<f64>() {
                    self.config.latitude = v;
                    self.recompute();
                    self.persist();
                }
                self.lat_text = s;
            }
            Message::SetLongitude(s) => {
                if let Ok(v) = s.trim().parse::<f64>() {
                    self.config.longitude = v;
                    self.recompute();
                    self.persist();
                }
                self.lon_text = s;
            }
            Message::SetLocationName(s) => {
                self.config.location_name = s;
                self.persist();
            }
            Message::SetMethod(i) => {
                self.config.method = CalcMethod::from_index(i);
                self.recompute();
                self.persist();
            }
            Message::SetMadhab(i) => {
                self.config.madhab = MadhabPref::from_index(i);
                self.recompute();
                self.persist();
            }
            Message::ToggleAdhan(i, b) => {
                if i < self.config.adhan_enabled.len() {
                    self.config.adhan_enabled[i] = b;
                    self.persist();
                }
            }
            Message::SetVolume(v) => {
                self.config.volume = v;
                self.audio.set_volume(v);
                self.persist();
            }
            Message::SetTimeFormat(i) => {
                self.config.time_format = TimeFormat::from_index(i);
                self.persist();
            }
            Message::SetLanguage(i) => {
                self.config.language = Language::from_index(i);
                self.persist();
            }
            Message::PickAdhanFile => {
                return cosmic::task::future(async {
                    let dialog = cosmic::dialog::file_chooser::open::Dialog::new()
                        .title("Select adhan audio file");
                    match dialog.open_file().await {
                        Ok(response) => Message::AdhanFilePicked(response.url().to_file_path().ok()),
                        Err(_) => Message::AdhanFilePicked(None),
                    }
                });
            }
            Message::AdhanFilePicked(path) => {
                if let Some(path) = path {
                    self.config.adhan_path = Some(path);
                    self.persist();
                }
            }
            Message::ConfigUpdated(config) => {
                if config != self.config {
                    self.config = config;
                    self.lat_text = format!("{}", self.config.latitude);
                    self.lon_text = format!("{}", self.config.longitude);
                    self.audio.set_volume(self.config.volume);
                    self.recompute();
                }
            }
            Message::PlaybackUpdated(playback) => {
                if playback != self.playback {
                    self.playback = playback;
                    self.apply_playback();
                }
            }
            Message::TogglePopup => {
                return if let Some(p) = self.popup.take() {
                    destroy_popup(p)
                } else {
                    self.now = prayer::now_utc();
                    self.page = Page::Main;
                    self.ensure_schedule();

                    let new_id = Id::unique();
                    self.popup = Some(new_id);
                    let mut popup_settings = self.core.applet.get_popup_settings(
                        self.core.main_window_id().unwrap(),
                        new_id,
                        None,
                        None,
                        None,
                    );
                    popup_settings.positioner.size_limits = Limits::NONE
                        .max_width(400.0)
                        .min_width(360.0)
                        .min_height(200.0)
                        .max_height(1080.0);
                    get_popup(popup_settings)
                };
            }
            Message::PopupClosed(id) => {
                if self.popup.as_ref() == Some(&id) {
                    self.popup = None;
                }
            }
        }
        Task::none()
    }

    fn view(&self) -> Element<'_, Self::Message> {
        let applet = &self.core.applet;
        let suggested = applet.suggested_size(true);
        let (pad_major, pad_minor) = applet.suggested_padding(true);
        let (h_pad, v_pad) = if applet.is_horizontal() {
            (pad_major, pad_minor)
        } else {
            (pad_minor, pad_major)
        };

        // Fill the panel's thickness so the whole panel height is clickable
        // (reachable at the screen edge), matching the built-in applets.
        let label = self.panel_label();
        let mut btn = button::custom(container(applet.text(label)).center_y(Length::Fill))
            .padding([0, h_pad])
            .on_press_down(Message::TogglePopup)
            .class(cosmic::theme::Button::AppletIcon);
        btn = if applet.is_horizontal() {
            btn.height(Length::Fixed(f32::from(suggested.1 + 2 * v_pad)))
        } else {
            btn.width(Length::Fixed(f32::from(suggested.0 + 2 * h_pad)))
        };

        // While the adhan is sounding (scheduled or a manual test), show a stop
        // control in the panel so it can be cancelled without opening the popup.
        let content: Element<'_, Self::Message> = if self.playing.is_some() || self.testing {
            let stop = applet
                .icon_button("media-playback-stop-symbolic")
                .on_press(Message::StopAdhan);
            row![btn, stop].align_y(Alignment::Center).into()
        } else {
            btn.into()
        };

        cosmic::widget::autosize::autosize(content, AUTOSIZE_ID.clone()).into()
    }

    fn view_window(&self, _id: Id) -> Element<'_, Self::Message> {
        let content = match self.page {
            Page::Main => self.main_view(),
            Page::Settings => self.settings_view(),
        };
        self.core.applet.popup_container(content).into()
    }

    fn style(&self) -> Option<cosmic::iced::theme::Style> {
        Some(cosmic::applet::style())
    }
}

/// Place `leading` on the start edge and `trailing` on the end edge, filling the
/// width. Mirrors for RTL so `leading` ends up on the right.
fn ends<'a>(
    rtl: bool,
    leading: Element<'a, Message>,
    trailing: Element<'a, Message>,
) -> Element<'a, Message> {
    let (a, b) = if rtl {
        (trailing, leading)
    } else {
        (leading, trailing)
    };
    row![a, space::horizontal().width(Length::Fill), b]
        .align_y(Alignment::Center)
        .width(Length::Fill)
        .into()
}

/// A tight `leading trailing` pair, mirrored for RTL.
fn pair<'a>(
    rtl: bool,
    leading: Element<'a, Message>,
    trailing: Element<'a, Message>,
    gap: u16,
) -> Element<'a, Message> {
    let (a, b) = if rtl {
        (trailing, leading)
    } else {
        (leading, trailing)
    };
    row![a, b].spacing(gap).align_y(Alignment::Center).into()
}

/// A settings row: label on the leading edge, control on the trailing edge.
fn item<'a>(
    rtl: bool,
    gap: u16,
    label: &'a str,
    control: Element<'a, Message>,
) -> Element<'a, Message> {
    let label_el: Element<'a, Message> = text::body(label).into();
    let (a, b) = if rtl {
        (control, label_el)
    } else {
        (label_el, control)
    };
    row![a, space::horizontal().width(Length::Fill), b]
        .spacing(gap)
        .align_y(Alignment::Center)
        .width(Length::Fill)
        .into()
}

/// A titled settings card containing the given item rows.
fn section<'a>(
    rtl: bool,
    spacing: Spacing,
    title: &'a str,
    items: Vec<Element<'a, Message>>,
) -> Element<'a, Message> {
    let align = if rtl { Alignment::End } else { Alignment::Start };
    let mut col = column![text::caption(title).class(cosmic::theme::Text::Accent)]
        .spacing(spacing.space_xs)
        .width(Length::Fill)
        .align_x(align);
    for it in items {
        col = col.push(it);
    }
    container(col)
        .padding(spacing.space_s)
        .width(Length::Fill)
        .class(cosmic::theme::Container::Card)
        .into()
}

fn prayer_row<'a>(
    rtl: bool,
    name: &'a str,
    time: &str,
    state: RowState,
    gap: u16,
) -> Element<'a, Message> {
    let accent = matches!(state, RowState::Current);
    let name_el: Element<'a, Message> = if accent {
        text::body(name).class(cosmic::theme::Text::Accent).into()
    } else {
        text::body(name).into()
    };
    let time_el: Element<'a, Message> = if matches!(state, RowState::Current) {
        text::body(time.to_string())
            .class(cosmic::theme::Text::Accent)
            .into()
    } else {
        text::body(time.to_string()).into()
    };
    let marker_el: Element<'a, Message> = text::body(if accent { "•  " } else { "    " })
        .class(if accent {
            cosmic::theme::Text::Accent
        } else {
            cosmic::theme::Text::Default
        })
        .into();

    let name_group = pair(rtl, marker_el, name_el, 0);
    let (a, b) = if rtl {
        (time_el, name_group)
    } else {
        (name_group, time_el)
    };
    row![a, space::horizontal().width(Length::Fill), b]
        .spacing(gap)
        .align_y(Alignment::Center)
        .width(Length::Fill)
        .padding([4, 4])
        .into()
}
