// SPDX-License-Identifier: GPL-3.0-only

use std::path::PathBuf;
use std::sync::LazyLock;
use std::time::Duration as StdDuration;

use cosmic::app::{Core, Task};
use cosmic::cosmic_config::{self, CosmicConfigEntry};
use cosmic::iced::platform_specific::shell::wayland::commands::popup::{destroy_popup, get_popup};
use cosmic::iced::widget::{column, row};
use cosmic::iced::{window::Id, Alignment, Length, Limits, Subscription};
use cosmic::widget::{button, container, divider, dropdown, icon, settings, slider, space, text, text_input, toggler};
use cosmic::{theme, Element};

use salah::prelude::{DateTime, Duration, Utc};

use crate::audio::AudioHandle;
use crate::config::{CalcMethod, Config, Language, MadhabPref, TimeFormat};
use crate::i18n;
use crate::prayer::{self, RowState, Schedule, Slot};

static AUTOSIZE_ID: LazyLock<cosmic::widget::Id> =
    LazyLock::new(|| cosmic::widget::Id::new("ppt-panel-autosize"));

const SNOOZE: Duration = Duration::minutes(5);

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

    fn start_play(&mut self, slot: Slot) {
        self.playing = Some(slot);
        if let Some(path) = self.config.resolved_adhan_path() {
            self.audio.play(path, self.config.volume);
        } else {
            tracing::info!(prayer = slot.name(), "adhan time (no audio file configured)");
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
        let Some(schedule) = self.schedule.as_ref() else {
            return "Prayer times".to_string();
        };
        let status = schedule.status(self.now, lang);
        format!(
            "{} {}",
            status.next_label,
            prayer::format_countdown(status.countdown, lang)
        )
    }

    fn main_view(&self) -> Element<'_, Message> {
        let spacing = theme::active().cosmic().spacing;
        let lang = self.config.language;
        let s = i18n::strings(lang);

        let Some(schedule) = self.schedule.as_ref() else {
            return container(text::body(s.unavailable)).padding(16).into();
        };

        let status = schedule.status(self.now, lang);
        let pattern = self.config.time_format_pattern();

        let header = column![
            text::body(self.config.location_name.clone()),
            text::title4(prayer::hijri_date_string(lang)),
        ]
        .spacing(2);

        let hero_inner = column![
            text::caption(s.current_prayer).class(cosmic::theme::Text::Accent),
            row_between(
                text::title3(status.current_label.clone()).into(),
                column![
                    text::title2(prayer::format_countdown(status.countdown, lang))
                        .class(cosmic::theme::Text::Accent),
                    text::caption(s.time_left),
                ]
                .align_x(Alignment::End)
                .into(),
            ),
            divider::horizontal::default(),
            row_between(
                row![
                    text::caption(s.next),
                    text::body(status.next_label.clone()),
                ]
                .spacing(spacing.space_xs)
                .align_y(Alignment::Center)
                .into(),
                text::body(i18n::localize_time(
                    &schedule.next_time_string(&status, pattern),
                    lang
                ))
                .into(),
            ),
        ]
        .spacing(spacing.space_xs);

        let hero = container(hero_inner)
            .padding(spacing.space_s)
            .width(Length::Fill)
            .class(cosmic::theme::Container::Card);

        let mut list = column![].spacing(spacing.space_xxs);
        for slot in Slot::ALL {
            let state = schedule.row_state(slot, &status, self.now);
            let time_str = i18n::localize_time(&schedule.local_time_string(slot, pattern), lang);
            list = list.push(prayer_row(
                slot.name_localized(lang),
                &time_str,
                state,
                spacing.space_xs,
            ));
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
                self.config.method.label(),
                self.config.madhab.label()
            ))
            .into()
        };
        let footer = row_between(gear.into(), footer_right);

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
        let s = i18n::strings(lang);

        let header = row![
            button::icon(icon::from_name("go-previous-symbolic"))
                .on_press(Message::CloseSettings),
            text::title4(s.settings),
        ]
        .spacing(spacing.space_s)
        .align_y(Alignment::Center);

        let method_opts: Vec<&str> = CalcMethod::ALL.iter().map(|m| m.label()).collect();
        let madhab_opts: Vec<&str> = MadhabPref::ALL.iter().map(|m| m.label()).collect();
        let time_opts: Vec<&str> = TimeFormat::ALL.iter().map(|t| t.label()).collect();
        let lang_opts: Vec<&str> = Language::ALL.iter().map(|l| l.label()).collect();

        let location = settings::section()
            .title(s.location)
            .add(settings::item(
                s.location_name,
                text_input("", &self.config.location_name).on_input(Message::SetLocationName),
            ))
            .add(settings::item(
                s.latitude,
                text_input("", &self.lat_text).on_input(Message::SetLatitude),
            ))
            .add(settings::item(
                s.longitude,
                text_input("", &self.lon_text).on_input(Message::SetLongitude),
            ));

        let calculation = settings::section()
            .title(s.calculation)
            .add(settings::item(
                s.method,
                dropdown(method_opts, Some(self.config.method.index()), Message::SetMethod),
            ))
            .add(settings::item(
                s.madhab,
                dropdown(madhab_opts, Some(self.config.madhab.index()), Message::SetMadhab),
            ));

        let mut adhan = settings::section().title(s.adhan);
        for slot in Slot::ALL {
            let i = slot.index();
            let enabled = self.config.adhan_enabled[i];
            adhan = adhan.add(settings::item(
                slot.name_localized(lang),
                toggler(enabled).on_toggle(move |b| Message::ToggleAdhan(i, b)),
            ));
        }
        let test_button: Element<'_, Message> = if self.testing {
            button::destructive(s.stop).on_press(Message::StopAdhan).into()
        } else {
            button::standard(s.play_test).on_press(Message::TestAdhan).into()
        };
        let adhan_name = self
            .config
            .adhan_path
            .as_ref()
            .and_then(|p| p.file_name())
            .map(|f| f.to_string_lossy().into_owned())
            .unwrap_or_else(|| s.default_file.to_string());
        let adhan = adhan
            .add(settings::item(
                s.volume,
                slider(0.0..=1.0, self.config.volume, Message::SetVolume).step(0.05f32),
            ))
            .add(settings::item(
                s.adhan_file,
                row![
                    text::body(adhan_name),
                    button::standard(s.choose).on_press(Message::PickAdhanFile),
                ]
                .spacing(spacing.space_s)
                .align_y(Alignment::Center),
            ))
            .add(settings::item(s.test_adhan, test_button))
            .add_maybe(
                self.config
                    .resolved_adhan_path()
                    .is_none()
                    .then(|| settings::item("", text::caption(s.no_audio_file))),
            );

        let display = settings::section()
            .title(s.display)
            .add(settings::item(
                s.time_format,
                dropdown(time_opts, Some(self.config.time_format.index()), Message::SetTimeFormat),
            ))
            .add(settings::item(
                s.language,
                dropdown(lang_opts, Some(self.config.language.index()), Message::SetLanguage),
            ));

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

        let now = prayer::now_utc();
        let mut app = PrayerApplet {
            core,
            popup: None,
            page: Page::Main,
            lat_text: format!("{}", config.latitude),
            lon_text: format!("{}", config.longitude),
            config,
            config_handle,
            audio: AudioHandle::spawn(),
            schedule: None,
            now,
            last_tick: now,
            playing: None,
            snooze: None,
            testing: false,
        };
        app.recompute();
        (app, Task::none())
    }

    fn on_close_requested(&self, id: Id) -> Option<Message> {
        Some(Message::PopupClosed(id))
    }

    fn subscription(&self) -> Subscription<Self::Message> {
        cosmic::iced::time::every(StdDuration::from_secs(1)).map(|_| Message::Tick)
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
                self.playing = None;
                self.snooze = None;
                self.testing = false;
                self.audio.stop();
            }
            Message::Snooze => {
                self.audio.stop();
                if let Some(slot) = self.playing.take() {
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
        let label = self.panel_label();
        let btn = button::custom(self.core.applet.text(label))
            .padding([0, self.core.applet.suggested_padding(true).0])
            .on_press_down(Message::TogglePopup)
            .class(cosmic::theme::Button::AppletIcon);

        cosmic::widget::autosize::autosize(btn, AUTOSIZE_ID.clone()).into()
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

/// A `left ......... right` row that fills the available width.
fn row_between<'a>(left: Element<'a, Message>, right: Element<'a, Message>) -> Element<'a, Message> {
    row![left, space::horizontal().width(Length::Fill), right]
        .align_y(Alignment::Center)
        .width(Length::Fill)
        .into()
}

fn prayer_row<'a>(name: &'a str, time: &str, state: RowState, gap: u16) -> Element<'a, Message> {
    let (name_widget, time_widget) = match state {
        RowState::Current => (
            text::body(name).class(cosmic::theme::Text::Accent),
            text::body(time.to_string()).class(cosmic::theme::Text::Accent),
        ),
        RowState::Next => (
            text::body(name).class(cosmic::theme::Text::Accent),
            text::body(time.to_string()),
        ),
        RowState::Passed | RowState::Upcoming => {
            (text::body(name), text::body(time.to_string()))
        }
    };

    let marker = match state {
        RowState::Current | RowState::Next => "•  ",
        _ => "    ",
    };

    row![
        text::body(marker).class(if matches!(state, RowState::Current | RowState::Next) {
            cosmic::theme::Text::Accent
        } else {
            cosmic::theme::Text::Default
        }),
        name_widget,
        space::horizontal().width(Length::Fill),
        time_widget,
    ]
    .spacing(gap)
    .align_y(Alignment::Center)
    .width(Length::Fill)
    .padding([4, 4])
    .into()
}
