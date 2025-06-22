pub mod periodic_cleaner;
pub mod responder;
pub mod watcher;

#[derive(Clone, Debug)]
pub enum WorkerType {
    SymlinkCleaners,
    Watchers,
    Responder,
}
