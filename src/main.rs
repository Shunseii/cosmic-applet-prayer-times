// SPDX-License-Identifier: GPL-3.0-only

mod app;
mod audio;
mod config;
mod i18n;
mod prayer;

fn main() -> cosmic::iced::Result {
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .init();

    // Start the applet's event loop with `()` as the application's flags.
    cosmic::applet::run::<app::PrayerApplet>(())
}
