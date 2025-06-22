use notify::Event;

pub mod periodic_cleaner;
pub mod responder;
pub mod watcher;

// Message ////////////////////////////////////////////////////////////////////

/// Message to be sent throgh the crossbeam_channel.
#[derive(Clone, Debug)]
pub enum Message {
    CreateNecessaryDirs,
    SymlinkCleanAll,
    SymlinkCleanDir(usize, usize),
    SymlinkCreateAll,
    Watch(WatchEvent),
    Shutdown,
}

/// Used in `Message::Watch(WatchEvent)`.
/// Provides needed additional info for the responder and its invoked symlinker actions.
#[derive(Clone, Debug)]
pub struct WatchEvent {
    pub rule_idx: usize,
    pub watch_idx: usize,
    pub event: Event,
}
