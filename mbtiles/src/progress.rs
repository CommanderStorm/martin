//! Terminal progress indicators for long-running CLI operations.

/// Whether progress should be rendered to stderr.
///
/// Progress is only drawn for an interactive terminal, so piped or captured output stays clean.
#[cfg(feature = "cli")]
fn is_interactive() -> bool {
    use std::io::IsTerminal as _;
    std::io::stderr().is_terminal()
}

/// A determinate progress bar tracking how many tiles have been copied.
///
/// Copying is split per zoom level, and the bar advances by each zoom's tile count as that zoom completes.
/// Zoom levels hold very different tile counts, so weighting each step by its own count keeps the bar proportional to the work done rather than to the number of zoom levels.
/// The bar is cleared when dropped, so it disappears on both success and early return.
/// It is a no-op when the `cli` feature is disabled or when stderr is not a terminal.
pub(crate) struct TileBar {
    #[cfg(feature = "cli")]
    bar: indicatif::ProgressBar,
}

impl TileBar {
    /// Creates a bar spanning `total_tiles`, labelled with `message`.
    pub(crate) fn new(total_tiles: u64, message: &'static str) -> Self {
        #[cfg(feature = "cli")]
        {
            use std::time::Duration;

            use indicatif::{ProgressBar, ProgressStyle};

            let bar = if is_interactive() {
                let bar = ProgressBar::new(total_tiles);
                bar.set_style(
                    ProgressStyle::default_bar()
                        .template("{elapsed_precise} -> eta: {eta} [{bar:40.cyan/blue} {percent}%] {pos}/{human_len} tiles ({per_sec}) | {msg}")
                        .expect("valid progress bar template")
                        .progress_chars("█▓▒░ "),
                );
                bar.set_message(message);
                bar.enable_steady_tick(Duration::from_millis(120));
                bar
            } else {
                ProgressBar::hidden()
            };
            Self { bar }
        }
        #[cfg(not(feature = "cli"))]
        {
            let _ = (total_tiles, message);
            Self {}
        }
    }

    /// Advances the bar by `tiles`.
    pub(crate) fn inc(&self, tiles: u64) {
        #[cfg(feature = "cli")]
        self.bar.inc(tiles);
        #[cfg(not(feature = "cli"))]
        let _ = tiles;
    }
}

#[cfg(feature = "cli")]
impl Drop for TileBar {
    fn drop(&mut self) {
        self.bar.finish_and_clear();
    }
}

/// An animated stderr spinner shown while an unmeasurable operation is in progress.
///
/// Some copy variants (applying or generating a patch) run as a single opaque SQL statement whose row progress is not observable, so the spinner only signals that work is ongoing.
/// It is cleared when dropped, and is a no-op when the `cli` feature is disabled or when stderr is not a terminal.
pub(crate) struct Spinner {
    #[cfg(feature = "cli")]
    bar: indicatif::ProgressBar,
}

impl Spinner {
    /// Starts a spinner labelled with `message`.
    pub(crate) fn start(message: &'static str) -> Self {
        #[cfg(feature = "cli")]
        {
            use std::time::Duration;

            use indicatif::{ProgressBar, ProgressStyle};

            let bar = if is_interactive() {
                let bar = ProgressBar::new_spinner();
                bar.set_style(
                    ProgressStyle::default_spinner()
                        .template("{spinner} {elapsed_precise} {msg}")
                        .expect("valid spinner template"),
                );
                bar.set_message(message);
                bar.enable_steady_tick(Duration::from_millis(120));
                bar
            } else {
                ProgressBar::hidden()
            };
            Self { bar }
        }
        #[cfg(not(feature = "cli"))]
        {
            let _ = message;
            Self {}
        }
    }
}

#[cfg(feature = "cli")]
impl Drop for Spinner {
    fn drop(&mut self) {
        self.bar.finish_and_clear();
    }
}

#[cfg(test)]
mod tests {
    use crate::progress::{Spinner, TileBar};

    #[test]
    fn tile_bar_without_terminal_is_a_noop() {
        let bar = TileBar::new(100, "Copying tiles");
        bar.inc(50);
        bar.inc(50);
        drop(bar);
    }

    #[test]
    fn spinner_without_terminal_is_a_noop() {
        let spinner = Spinner::start("Copying tiles");
        drop(spinner);
    }
}
