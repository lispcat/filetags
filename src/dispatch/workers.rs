pub mod periodic_cleaner;
pub mod watcher;

#[derive(Clone, Debug)]
pub enum WorkerType {
    Cleaners,
    Watchers,
    Responder,
}
