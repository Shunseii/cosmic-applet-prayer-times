// SPDX-License-Identifier: GPL-3.0-only
//
// Prayer-time computation wrapper around the `salah` crate. We only surface the
// five obligatory prayers (Fajr, Dhuhr, Asr, Maghrib, Isha); Sunrise and Qiyam
// are intentionally skipped for the applet UI.

use hijri_date::HijriDate;
use salah::prelude::{
    Configuration, Coordinates, DateTime, Datelike, Duration, Local, NaiveDate, Prayer,
    PrayerTimes, Utc,
};

/// English transliteration of the Islamic (Hijri) months, 1-indexed via `month - 1`.
const HIJRI_MONTHS_EN: [&str; 12] = [
    "Muharram",
    "Safar",
    "Rabiʿ al-Awwal",
    "Rabiʿ al-Thani",
    "Jumada al-Ula",
    "Jumada al-Akhirah",
    "Rajab",
    "Shaʿban",
    "Ramadan",
    "Shawwal",
    "Dhuʾl-Qaʿdah",
    "Dhuʾl-Hijjah",
];

use crate::config::{Config, Language};
use crate::i18n;

/// The five obligatory daily prayers, in order.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Slot {
    Fajr,
    Dhuhr,
    Asr,
    Maghrib,
    Isha,
}

impl Slot {
    pub const ALL: [Slot; 5] = [
        Slot::Fajr,
        Slot::Dhuhr,
        Slot::Asr,
        Slot::Maghrib,
        Slot::Isha,
    ];

    pub fn index(self) -> usize {
        match self {
            Slot::Fajr => 0,
            Slot::Dhuhr => 1,
            Slot::Asr => 2,
            Slot::Maghrib => 3,
            Slot::Isha => 4,
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            Slot::Fajr => "Fajr",
            Slot::Dhuhr => "Dhuhr",
            Slot::Asr => "Asr",
            Slot::Maghrib => "Maghrib",
            Slot::Isha => "Isha",
        }
    }

    pub fn name_localized(self, lang: Language) -> &'static str {
        match lang {
            Language::English => self.name(),
            Language::Arabic => match self {
                Slot::Fajr => "الفجر",
                Slot::Dhuhr => "الظهر",
                Slot::Asr => "العصر",
                Slot::Maghrib => "المغرب",
                Slot::Isha => "العشاء",
            },
        }
    }

}

/// Per-row state used by the popup list.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RowState {
    Passed,
    Current,
    Next,
    Upcoming,
}

/// A computed day of prayer times (UTC instants) plus tomorrow's Fajr so the
/// "next prayer" can wrap past Isha.
pub struct Schedule {
    date: NaiveDate,
    times: [DateTime<Utc>; 5],
    sunrise: DateTime<Utc>,
    fajr_tomorrow: DateTime<Utc>,
}

/// Derived "what is current / what is next" view for a given instant.
pub struct Status {
    pub current_label: String,
    pub current_index: Option<usize>,
    pub next_label: String,
    pub next_index: Option<usize>,
    pub next_time: DateTime<Utc>,
    pub countdown: Duration,
}

impl Schedule {
    pub fn compute(config: &Config, date: NaiveDate) -> Result<Self, String> {
        let coords = Coordinates::new(config.latitude, config.longitude);
        let params = Configuration::with(config.method.to_salah(), config.madhab.to_salah());
        let times = PrayerTimes::new(date, coords, params);

        let resolved = [
            times.time(Prayer::Fajr),
            times.time(Prayer::Dhuhr),
            times.time(Prayer::Asr),
            times.time(Prayer::Maghrib),
            times.time(Prayer::Isha),
        ];

        Ok(Self {
            date,
            times: resolved,
            sunrise: times.time(Prayer::Sunrise),
            fajr_tomorrow: times.time(Prayer::FajrTomorrow),
        })
    }

    pub fn date(&self) -> NaiveDate {
        self.date
    }

    pub fn local_time_string(&self, slot: Slot, pattern: &str) -> String {
        self.times[slot.index()]
            .with_timezone(&Local)
            .format(pattern)
            .to_string()
    }

    fn local_time_string_for(time: DateTime<Utc>, pattern: &str) -> String {
        time.with_timezone(&Local).format(pattern).to_string()
    }

    /// Sunrise (informational; marks the end of the Fajr window).
    pub fn sunrise_string(&self, pattern: &str) -> String {
        Self::local_time_string_for(self.sunrise, pattern)
    }

    pub fn sunrise_passed(&self, now: DateTime<Utc>) -> bool {
        self.sunrise <= now
    }

    /// Determine current/next prayer and the countdown to next, relative to `now`.
    pub fn status(&self, now: DateTime<Utc>, lang: Language) -> Status {
        // First prayer today strictly after `now`.
        let next_idx = (0..5).find(|&i| self.times[i] > now);

        match next_idx {
            Some(i) => {
                let current_index = if i == 0 { None } else { Some(i - 1) };
                // After sunrise (before Dhuhr) the current period is sunrise, not Fajr.
                let current_label = if current_index == Some(Slot::Fajr.index())
                    && now >= self.sunrise
                {
                    i18n::strings(lang).sunrise.to_string()
                } else {
                    match current_index {
                        Some(c) => Slot::ALL[c].name_localized(lang).to_string(),
                        // Before Fajr: the active period is last night's Isha.
                        None => Slot::Isha.name_localized(lang).to_string(),
                    }
                };
                // Fajr's window ends at sunrise, not at the next prayer, so during
                // Fajr the "next" event and countdown both target sunrise.
                let fajr_before_sunrise =
                    current_index == Some(Slot::Fajr.index()) && now < self.sunrise;
                let (next_label, next_time) = if fajr_before_sunrise {
                    (i18n::strings(lang).sunrise.to_string(), self.sunrise)
                } else {
                    (Slot::ALL[i].name_localized(lang).to_string(), self.times[i])
                };
                Status {
                    current_label,
                    current_index,
                    next_label,
                    next_index: Some(i),
                    next_time,
                    countdown: next_time.signed_duration_since(now),
                }
            }
            // After Isha: next is tomorrow's Fajr; Isha is the active prayer.
            None => Status {
                current_label: Slot::Isha.name_localized(lang).to_string(),
                current_index: Some(Slot::Isha.index()),
                next_label: Slot::Fajr.name_localized(lang).to_string(),
                next_index: None,
                next_time: self.fajr_tomorrow,
                countdown: self.fajr_tomorrow.signed_duration_since(now),
            },
        }
    }

    pub fn next_time_string(&self, status: &Status, pattern: &str) -> String {
        Self::local_time_string_for(status.next_time, pattern)
    }

    pub fn row_state(&self, slot: Slot, status: &Status, now: DateTime<Utc>) -> RowState {
        let i = slot.index();
        if status.current_index == Some(i) {
            RowState::Current
        } else if status.next_index == Some(i) {
            RowState::Next
        } else if self.times[i] <= now {
            RowState::Passed
        } else {
            RowState::Upcoming
        }
    }

    /// Indices of the five prayers whose time falls in `(last, now]` — i.e. those
    /// that just became due since the previous tick. Used to fire the adhan once.
    pub fn crossings(&self, last: DateTime<Utc>, now: DateTime<Utc>) -> Vec<Slot> {
        Slot::ALL
            .into_iter()
            .filter(|s| {
                let t = self.times[s.index()];
                t > last && t <= now
            })
            .collect()
    }
}

/// Format a positive duration as `Xh XXm` (e.g. "1h 23m"), `XXm` under an hour,
/// or "<1m" under a minute. Localizes units and digits for Arabic.
pub fn format_countdown(d: Duration, lang: Language) -> String {
    let total = d.num_seconds().max(0);
    let h = total / 3600;
    let m = (total % 3600) / 60;
    let (hu, mu) = if lang.is_rtl() { ("س", "د") } else { ("h", "m") };
    let s = if h > 0 {
        format!("{h}{hu} {m:02}{mu}")
    } else if m > 0 {
        format!("{m}{mu}")
    } else {
        return if lang.is_rtl() {
            "< ١د".to_string()
        } else {
            "<1m".to_string()
        };
    };
    i18n::digits(&s, lang)
}

/// Today's date in the local timezone.
pub fn today_local() -> NaiveDate {
    Local::now().date_naive()
}

/// Current instant in UTC.
pub fn now_utc() -> DateTime<Utc> {
    Utc::now()
}

/// Hijri (Umm al-Qura) date for the popup header, localized. English, e.g.
/// "Saturday, 14 Dhuʾl-Qaʿdah 1447 AH"; Arabic, e.g. "السبت، ١٤ ذو القعدة ١٤٤٧ هـ".
/// Falls back to the Gregorian date if the conversion fails (the Umm al-Qura
/// table is bounded to ~1937–2077).
pub fn hijri_date_string(lang: Language) -> String {
    let now = Local::now();
    match HijriDate::from_gr(now.year() as usize, now.month() as usize, now.day() as usize) {
        Ok(h) => match lang {
            Language::English => {
                let month = HIJRI_MONTHS_EN
                    .get(h.month().saturating_sub(1))
                    .copied()
                    .unwrap_or("");
                format!("{}, {} {} {} AH", h.day_name_en(), h.day(), month, h.year())
            }
            Language::Arabic => format!(
                "{}، {} {} {} هـ",
                h.day_name(),
                i18n::digits(&h.day().to_string(), lang),
                h.month_name(),
                i18n::digits(&h.year().to_string(), lang),
            ),
        },
        Err(_) => now.format("%A, %-d %B").to_string(),
    }
}
