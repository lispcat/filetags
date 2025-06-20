use std::sync::Arc;

use anyhow::Context;
use channels::{responder::start_responder, watcher::start_watchers};
use crossbeam_channel::{Receiver, Sender};
use tracing::debug;

mod args;
mod channels;
mod config;
mod logger;
mod utils;

// re-export
pub use args::*;
pub use channels::*;
pub use config::*;
pub use logger::*;
pub use utils::*;

use crate::actions::{
    cleaning::{periodic_cleaner::start_symlink_cleaners, query_symlink_clean_all},
    filesystem_asserts::query_create_necessary_dirs,
    symlinking::query_symlink_create_all,
};

// TODO:
// - prevent recursive searching when LinkDir is within WatchDir or symlinking dirs.

/// The default run command.
pub fn run() -> anyhow::Result<()> {
    let args = Args::parse();
    let (tx, rx) = crossbeam_channel::unbounded::<Message>();
    run_with_args(args, tx, rx)
}

/// Run the program with args, tx, and rx.
pub fn run_with_args(args: Args, tx: Sender<Message>, rx: Receiver<Message>) -> anyhow::Result<()> {
    // create a Config from Args
    let config: Arc<Config> = Config::create(&args)?;
    let _logger = Logger::new();

    run_with_config(config, tx, rx, None::<fn()>)
}

/// Run the program with config, tx, rx, and optionally test_hook.
pub fn run_with_config<F: Fn() + Send + 'static>(
    config: Arc<Config>,
    tx: Sender<Message>,
    rx: Receiver<Message>,
    test_hook: Option<F>,
) -> anyhow::Result<()> {
    span_enter!(DEBUG, "running");
    debug!("Config: {:?}", config);

    // start responder
    let responder_handle = start_responder(rx, &config).context("starting responder")?;

    // create all necessary dirs
    query_create_necessary_dirs(&tx)?;

    // clean all broken or innapropriate links in link_dirs
    query_symlink_clean_all(&tx)?;

    // maybe create symlinks as appropriate
    query_symlink_create_all(&tx)?;

    // start all link cleaners
    start_symlink_cleaners(&tx, &config).context("starting cleaners")?;

    // setup all watchers
    start_watchers(&tx, &config).context("starting watchers")?;

    // maybe run test hook (for integration tests)
    test_hook.inspect(|hook_fn| {
        span_enter!(DEBUG, "test_hook");
        hook_fn();
    });

    // block this thread until the responder thread completes
    responder_handle
        .join()
        .expect("failed to join respender thread")?;

    Ok(())
}
