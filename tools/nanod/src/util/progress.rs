use std::time::Duration;

use espflash::target::ProgressCallbacks;
use indicatif::{ProgressBar, ProgressStyle};

/// Progress callback that wraps an indicatif progress bar for espflash operations.
pub struct FlashProgress {
    pb: Option<ProgressBar>,
    msg: String,
}

impl FlashProgress {
    pub fn new(msg: &str) -> Self {
        Self {
            pb: None,
            msg: msg.to_string(),
        }
    }
}

impl ProgressCallbacks for FlashProgress {
    fn init(&mut self, _addr: u32, total: usize) {
        let pb = ProgressBar::new(total as u64);
        pb.set_style(
            ProgressStyle::default_bar()
                .template("{msg} [{bar:40.cyan/blue}] {bytes}/{total_bytes} ({eta})")
                .expect("invalid progress template")
                .progress_chars("=> "),
        );
        pb.set_message(self.msg.clone());
        self.pb = Some(pb);
    }

    fn update(&mut self, current: usize) {
        if let Some(pb) = &self.pb {
            pb.set_position(current as u64);
        }
    }

    fn verifying(&mut self) {
        if let Some(pb) = &self.pb {
            pb.set_message("Verifying...");
        }
    }

    fn finish(&mut self, skipped: bool) {
        if let Some(pb) = self.pb.take() {
            if skipped {
                pb.finish_with_message("Skipped (already up to date)");
            } else {
                pb.finish_with_message("Done");
            }
        }
    }
}

pub fn spinner(msg: &str) -> ProgressBar {
    let pb = ProgressBar::new_spinner();
    pb.set_style(
        ProgressStyle::default_spinner()
            .template("{spinner:.green} {msg}")
            .expect("invalid spinner template"),
    );
    pb.set_message(msg.to_string());
    pb.enable_steady_tick(Duration::from_millis(100));
    pb
}
