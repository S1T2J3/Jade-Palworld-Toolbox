//! Native, read-only Palworld save discovery and terminal import.
//! Implementation is intentionally isolated from the UI and never writes game saves.

mod container;
mod discovery;
mod props;
mod terminal;
mod wgs;

pub use discovery::{scan_game_saves, scan_from_locations, GameWorld, ScanResult};
pub use terminal::{read_terminal, read_terminal_files, PlayerFile, TerminalOwner, TerminalRecord, TerminalResult};

pub type SaveResult<T> = Result<T, String>;
