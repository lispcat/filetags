pub mod actions;
pub mod responder;
pub mod watcher;

use notify::Event;

/// Message to be sent throgh the crossbeam_channel.
#[derive(Clone, Debug)]
pub enum Message {
    Watch(WatchEvent),
    CleanDir(usize, usize),
    CleanAll,
    Shutdown,
}

#[derive(Clone, Debug)]
pub struct WatchEvent {
    pub rule_idx: usize,
    pub watch_idx: usize,
    pub event: Event,
}
