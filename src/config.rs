// SPDX-License-Identifier: GPL-3.0-only
//
// Persisted configuration, stored via `cosmic-config`. The calculation method,
// madhab, and time format are mirrored as our own serde-serializable enums
// (the `salah` types don't implement serde) and converted at compute time.

use std::path::PathBuf;

use cosmic::cosmic_config::{self, cosmic_config_derive::CosmicConfigEntry, CosmicConfigEntry};
use salah::prelude::{Madhab as SalahMadhab, Method as SalahMethod};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CalcMethod {
    MuslimWorldLeague,
    Egyptian,
    Karachi,
    UmmAlQura,
    Dubai,
    MoonsightingCommittee,
    NorthAmerica,
    Kuwait,
    Qatar,
    Singapore,
    Tehran,
    Turkey,
    Other,
}

impl CalcMethod {
    pub const ALL: [CalcMethod; 13] = [
        CalcMethod::MuslimWorldLeague,
        CalcMethod::Egyptian,
        CalcMethod::Karachi,
        CalcMethod::UmmAlQura,
        CalcMethod::Dubai,
        CalcMethod::MoonsightingCommittee,
        CalcMethod::NorthAmerica,
        CalcMethod::Kuwait,
        CalcMethod::Qatar,
        CalcMethod::Singapore,
        CalcMethod::Tehran,
        CalcMethod::Turkey,
        CalcMethod::Other,
    ];

    pub fn label(self) -> &'static str {
        match self {
            CalcMethod::MuslimWorldLeague => "Muslim World League",
            CalcMethod::Egyptian => "Egyptian General Authority",
            CalcMethod::Karachi => "Karachi",
            CalcMethod::UmmAlQura => "Umm al-Qura (Makkah)",
            CalcMethod::Dubai => "Dubai",
            CalcMethod::MoonsightingCommittee => "Moonsighting Committee",
            CalcMethod::NorthAmerica => "North America (ISNA)",
            CalcMethod::Kuwait => "Kuwait",
            CalcMethod::Qatar => "Qatar",
            CalcMethod::Singapore => "Singapore",
            CalcMethod::Tehran => "Tehran",
            CalcMethod::Turkey => "Turkey (Diyanet)",
            CalcMethod::Other => "Other",
        }
    }

    pub fn to_salah(self) -> SalahMethod {
        match self {
            CalcMethod::MuslimWorldLeague => SalahMethod::MuslimWorldLeague,
            CalcMethod::Egyptian => SalahMethod::Egyptian,
            CalcMethod::Karachi => SalahMethod::Karachi,
            CalcMethod::UmmAlQura => SalahMethod::UmmAlQura,
            CalcMethod::Dubai => SalahMethod::Dubai,
            CalcMethod::MoonsightingCommittee => SalahMethod::MoonsightingCommittee,
            CalcMethod::NorthAmerica => SalahMethod::NorthAmerica,
            CalcMethod::Kuwait => SalahMethod::Kuwait,
            CalcMethod::Qatar => SalahMethod::Qatar,
            CalcMethod::Singapore => SalahMethod::Singapore,
            CalcMethod::Tehran => SalahMethod::Tehran,
            CalcMethod::Turkey => SalahMethod::Turkey,
            CalcMethod::Other => SalahMethod::Other,
        }
    }

    pub fn index(self) -> usize {
        Self::ALL.iter().position(|m| *m == self).unwrap_or(0)
    }

    pub fn from_index(i: usize) -> Self {
        Self::ALL.get(i).copied().unwrap_or(CalcMethod::MuslimWorldLeague)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum MadhabPref {
    Shafi,
    Hanafi,
}

impl MadhabPref {
    pub const ALL: [MadhabPref; 2] = [MadhabPref::Shafi, MadhabPref::Hanafi];

    pub fn label(self) -> &'static str {
        match self {
            MadhabPref::Shafi => "Shafiʿi",
            MadhabPref::Hanafi => "Hanafi",
        }
    }

    pub fn to_salah(self) -> SalahMadhab {
        match self {
            MadhabPref::Shafi => SalahMadhab::Shafi,
            MadhabPref::Hanafi => SalahMadhab::Hanafi,
        }
    }

    pub fn index(self) -> usize {
        Self::ALL.iter().position(|m| *m == self).unwrap_or(0)
    }

    pub fn from_index(i: usize) -> Self {
        Self::ALL.get(i).copied().unwrap_or(MadhabPref::Shafi)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TimeFormat {
    Twelve,
    TwentyFour,
}

impl TimeFormat {
    pub const ALL: [TimeFormat; 2] = [TimeFormat::Twelve, TimeFormat::TwentyFour];

    pub fn label(self) -> &'static str {
        match self {
            TimeFormat::Twelve => "12-hour",
            TimeFormat::TwentyFour => "24-hour",
        }
    }

    pub fn pattern(self) -> &'static str {
        match self {
            TimeFormat::Twelve => "%-I:%M %p",
            TimeFormat::TwentyFour => "%H:%M",
        }
    }

    pub fn index(self) -> usize {
        Self::ALL.iter().position(|m| *m == self).unwrap_or(0)
    }

    pub fn from_index(i: usize) -> Self {
        Self::ALL.get(i).copied().unwrap_or(TimeFormat::Twelve)
    }
}

#[derive(Debug, Clone, PartialEq, CosmicConfigEntry)]
#[version = 1]
pub struct Config {
    pub latitude: f64,
    pub longitude: f64,
    pub method: CalcMethod,
    pub madhab: MadhabPref,
    /// Per-prayer adhan enable, ordered Fajr, Dhuhr, Asr, Maghrib, Isha.
    pub adhan_enabled: [bool; 5],
    pub volume: f32,
    pub time_format: TimeFormat,
    /// Custom adhan audio file. `None` falls back to a default-path lookup.
    pub adhan_path: Option<PathBuf>,
}

impl Default for Config {
    fn default() -> Self {
        // Default location: Toronto, Canada (ISNA is the conventional method
        // for North America).
        Self {
            latitude: 43.6532,
            longitude: -79.3832,
            method: CalcMethod::NorthAmerica,
            madhab: MadhabPref::Shafi,
            adhan_enabled: [true; 5],
            volume: 0.8,
            time_format: TimeFormat::Twelve,
            adhan_path: default_adhan_path(),
        }
    }
}

impl Config {
    pub fn time_format_pattern(&self) -> &'static str {
        self.time_format.pattern()
    }

    /// The adhan file to play: the configured path if set, else a default-path lookup.
    pub fn resolved_adhan_path(&self) -> Option<PathBuf> {
        self.adhan_path.clone().or_else(default_adhan_path)
    }
}

/// Look for a bundled/user-supplied adhan file in a couple of standard spots.
fn default_adhan_path() -> Option<PathBuf> {
    let mut candidates: Vec<PathBuf> = Vec::new();

    if let Ok(home) = std::env::var("HOME") {
        candidates.push(
            PathBuf::from(&home).join(".local/share/cosmic-applet-prayer-times/adhan.ogg"),
        );
    }
    candidates.push(PathBuf::from("assets/adhan.ogg"));

    candidates.into_iter().find(|p| p.exists())
}
