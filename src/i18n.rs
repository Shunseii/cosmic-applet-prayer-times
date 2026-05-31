// SPDX-License-Identifier: GPL-3.0-only
//
// Lightweight in-tree localization (English + Arabic). This switches displayed
// *content*; full RTL *layout* mirroring is a separate, larger task and is not
// done here (Arabic text still shapes/orders correctly within each label via
// cosmic-text's bidi support).

use crate::config::Language;

pub struct Strings {
    pub current_prayer: &'static str,
    pub time_left: &'static str,
    pub next: &'static str,
    pub settings: &'static str,
    pub location: &'static str,
    pub location_name: &'static str,
    pub latitude: &'static str,
    pub longitude: &'static str,
    pub calculation: &'static str,
    pub method: &'static str,
    pub madhab: &'static str,
    pub adhan: &'static str,
    pub volume: &'static str,
    pub adhan_file: &'static str,
    pub adhan_file_placeholder: &'static str,
    pub test_adhan: &'static str,
    pub play_test: &'static str,
    pub stop: &'static str,
    pub snooze: &'static str,
    pub display: &'static str,
    pub time_format: &'static str,
    pub language: &'static str,
    pub unavailable: &'static str,
    pub no_audio_file: &'static str,
    pub playing_label: &'static str,
}

pub fn strings(lang: Language) -> Strings {
    match lang {
        Language::English => Strings {
            current_prayer: "CURRENT PRAYER",
            time_left: "time left",
            next: "NEXT",
            settings: "Settings",
            location: "Location",
            location_name: "Location name",
            latitude: "Latitude",
            longitude: "Longitude",
            calculation: "Calculation",
            method: "Method",
            madhab: "Madhab",
            adhan: "Adhan",
            volume: "Volume",
            adhan_file: "Adhan file",
            adhan_file_placeholder: "path to audio file",
            test_adhan: "Test adhan",
            play_test: "Play test",
            stop: "Stop",
            snooze: "Snooze 5m",
            display: "Display",
            time_format: "Time format",
            language: "Language",
            unavailable: "Prayer times unavailable",
            no_audio_file: "No adhan file found",
            playing_label: "adhan",
        },
        Language::Arabic => Strings {
            current_prayer: "الصلاة الحالية",
            time_left: "المتبقّي",
            next: "التالية",
            settings: "الإعدادات",
            location: "الموقع",
            location_name: "اسم الموقع",
            latitude: "خط العرض",
            longitude: "خط الطول",
            calculation: "طريقة الحساب",
            method: "الطريقة",
            madhab: "المذهب",
            adhan: "الأذان",
            volume: "مستوى الصوت",
            adhan_file: "ملف الأذان",
            adhan_file_placeholder: "مسار الملف الصوتي",
            test_adhan: "اختبار الأذان",
            play_test: "تشغيل",
            stop: "إيقاف",
            snooze: "تأجيل ٥ د",
            display: "العرض",
            time_format: "تنسيق الوقت",
            language: "اللغة",
            unavailable: "تعذّر حساب أوقات الصلاة",
            no_audio_file: "لم يُعثر على ملف الأذان",
            playing_label: "أذان",
        },
    }
}

/// Convert Western digits in `s` to Arabic-Indic digits when `lang` is Arabic.
pub fn digits(s: &str, lang: Language) -> String {
    if !lang.is_rtl() {
        return s.to_string();
    }
    s.chars()
        .map(|c| match c {
            '0'..='9' => char::from_u32('٠' as u32 + (c as u32 - '0' as u32)).unwrap_or(c),
            _ => c,
        })
        .collect()
}

/// Localize a formatted clock time, e.g. "5:14 AM" -> "٥:١٤ ص" in Arabic.
pub fn localize_time(s: &str, lang: Language) -> String {
    if !lang.is_rtl() {
        return s.to_string();
    }
    let replaced = s.replace("AM", "ص").replace("PM", "م");
    digits(&replaced, lang)
}
