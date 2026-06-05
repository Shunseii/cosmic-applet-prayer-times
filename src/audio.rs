// SPDX-License-Identifier: GPL-3.0-only
//
// Adhan playback. Per `DESIGN.md` §3, playback lives behind a small command
// interface so a future systemd-user-service backend (Option B) can replace it
// without touching the UI. The rodio device handle is `!Send`, so it lives on a
// dedicated thread and is driven through an mpsc channel; the applet model only
// holds the (Send) sender.

use std::fs::File;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender};
use std::sync::Arc;
use std::thread;
use std::time::Duration;

use rodio::{Decoder, DeviceSinkBuilder, Player};

/// How often the audio thread checks whether the current adhan has finished
/// playing on its own. Keeps the UI's stop button from lingering after the
/// audio ends.
const FINISH_POLL: Duration = Duration::from_millis(200);

enum Command {
    Play { path: PathBuf, volume: f32 },
    Stop,
    #[allow(dead_code)] // for a future live volume slider in the settings view
    SetVolume(f32),
}

/// Handle to the audio thread. Cloning is cheap (shares the same channel).
#[derive(Clone)]
pub struct AudioHandle {
    tx: Sender<Command>,
    /// Set by the audio thread when playback ends on its own (not via `Stop`).
    /// The UI polls and clears it to reconcile its "adhan playing" state.
    finished: Arc<AtomicBool>,
}

impl AudioHandle {
    /// Spawn the audio thread and return a handle. If the audio device cannot be
    /// opened, playback commands become no-ops (logged once).
    pub fn spawn() -> Self {
        let (tx, rx) = mpsc::channel();
        let finished = Arc::new(AtomicBool::new(false));
        let finished_thread = finished.clone();
        thread::Builder::new()
            .name("adhan-audio".into())
            .spawn(move || audio_loop(rx, finished_thread))
            .expect("spawn audio thread");
        Self { tx, finished }
    }

    /// Returns `true` once if the adhan finished playing on its own since the
    /// last call, resetting the flag. `false` for a `Stop`-driven end.
    pub fn take_finished(&self) -> bool {
        self.finished.swap(false, Ordering::AcqRel)
    }

    pub fn play(&self, path: PathBuf, volume: f32) {
        let _ = self.tx.send(Command::Play { path, volume });
    }

    pub fn stop(&self) {
        let _ = self.tx.send(Command::Stop);
    }

    #[allow(dead_code)] // for a future live volume slider in the settings view
    pub fn set_volume(&self, volume: f32) {
        let _ = self.tx.send(Command::SetVolume(volume));
    }
}

fn audio_loop(rx: Receiver<Command>, finished: Arc<AtomicBool>) {
    let handle = match DeviceSinkBuilder::open_default_sink() {
        Ok(handle) => handle,
        Err(err) => {
            tracing::warn!(%err, "no audio device; adhan playback disabled");
            // Drain commands so senders don't see errors, but do nothing.
            while rx.recv().is_ok() {}
            return;
        }
    };

    let mut player: Option<Player> = None;

    loop {
        match rx.recv_timeout(FINISH_POLL) {
            Ok(Command::Play { path, volume }) => {
                if let Some(p) = player.take() {
                    p.stop();
                }
                match File::open(&path).map_err(|e| e.to_string()).and_then(|f| {
                    Decoder::try_from(f).map_err(|e| e.to_string())
                }) {
                    Ok(source) => {
                        let p = Player::connect_new(handle.mixer());
                        p.set_volume(volume);
                        p.append(source);
                        p.play();
                        player = Some(p);
                        finished.store(false, Ordering::Release);
                        tracing::info!(?path, "playing adhan");
                    }
                    Err(err) => {
                        tracing::warn!(?path, %err, "could not load adhan file");
                    }
                }
            }
            Ok(Command::Stop) => {
                if let Some(p) = player.take() {
                    p.stop();
                }
                // A manual stop is not a natural finish; the UI drives its own
                // state in that case.
                finished.store(false, Ordering::Release);
            }
            Ok(Command::SetVolume(volume)) => {
                if let Some(p) = &player {
                    p.set_volume(volume);
                }
            }
            Err(RecvTimeoutError::Timeout) => {}
            Err(RecvTimeoutError::Disconnected) => break,
        }

        // Detect playback that ended on its own so the UI can reset.
        if let Some(p) = &player {
            if p.empty() {
                player = None;
                finished.store(true, Ordering::Release);
            }
        }
    }
}
