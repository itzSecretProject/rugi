//! Disk usage: scans a folder and lists what takes the space, largest first.
//! Click a folder to open it; right-click an entry to copy its path.
//!
//! ```text
//! cargo run --release -p disk-usage [-- FOLDER]
//! cargo run --release -p disk-usage -- crates --screenshot out.png --sleep 500 --click rugi-ui
//! ```
//!
//! FOLDER defaults to the current folder. The screenshot flags are listed in
//! `examples/common/src/lib.rs`.

mod app;
mod scan;

use rugi::AppOptions;
use rugi_example_shot as shot;

fn main() -> shot::Result {
    let folder = shot::args().into_iter().next().filter(|a| !a.starts_with("--")).unwrap_or_else(|| ".".into());
    let root = std::fs::canonicalize(&folder).unwrap_or_else(|_| folder.into());
    let options = AppOptions { title: "Disk usage".into(), size: [1000, 680], ..Default::default() };
    shot::run(options, app::DiskUsage::new(root))
}
