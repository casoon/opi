//! `opi` — Operations Interface.
//!
//! This is a placeholder release that reserves the crate name. The tool itself
//! is not implemented yet; see the repository for what it is going to be.

use runemark::{ColorMode, Console, Tone};

const REPOSITORY: &str = env!("CARGO_PKG_REPOSITORY");
const VERSION: &str = env!("CARGO_PKG_VERSION");

fn main() {
    let console = Console::stdout(ColorMode::Auto);

    println!("{} {}", console.paint(Tone::Title, "opi"), VERSION);
    println!(
        "{}",
        console.paint(Tone::Muted, "Operations Interface — not implemented yet.")
    );
    println!();
    println!(
        "{}",
        console.paint(Tone::Info, format!("Follow along at {REPOSITORY}"))
    );
}
