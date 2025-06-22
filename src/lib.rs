use std::sync::Arc;

use anyhow::Context;
use clap::Parser;
use crossbeam_channel::{Receiver, Sender};
use systemd::daemon;
use tracing::debug;

mod args;
mod config;
mod logger;
mod symlinks;
mod utils;
mod workers;

// re-export
pub use args::*;
pub use config::*;
pub use logger::*;
pub use symlinks::*;
pub use utils::*;
pub use workers::*;

use crate::{
    cleaning::query_symlink_clean_all, filesystem::query_create_necessary_dirs,
    periodic_cleaner::start_symlink_cleaners, responder::start_responder,
    symlinking::query_symlink_create_all, watcher::start_watchers,
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
    debug!("Config: {:#?}", config);

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

    // if running as a systemd service, notify systemd that the service is ready
    if config.misc.systemd_service {
        daemon::notify(false, [(daemon::STATE_READY, "1")].iter())?;
    }

    // block this thread until the responder thread completes
    responder_handle
        .join()
        .expect("failed to join respender thread")?;

    Ok(())
}
