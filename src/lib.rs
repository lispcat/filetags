use std::sync::Arc;

use anyhow::Context;
use channels::{
    actions::cleaning::start_cleaners, responder::start_responder, watcher::start_watchers,
};
use crossbeam_channel::{Receiver, Sender};
use tracing::debug;

mod args;
mod channels;
mod config;
mod logger;
mod utils;

// re-export
pub use args::*;
pub use channels::Message;
pub use config::*;
pub use logger::*;
pub use utils::*;

// TODO:
// - prevent recursive searching when LinkDir is within WatchDir or symlinking dirs.

pub fn run() -> anyhow::Result<()> {
    let args = Args::parse();
    let (tx, rx) = crossbeam_channel::unbounded::<Message>();
    run_with_args(args, tx, rx)
}

pub fn run_with_args(args: Args, tx: Sender<Message>, rx: Receiver<Message>) -> anyhow::Result<()> {
    // create a Config from Args
    let config: Arc<Config> = Config::create(&args)?;
    let _logger = Logger::new();

    run_with_config(config, tx, rx, None::<fn()>)
}

// TODO: make run_with_args require tx and rx channels
pub fn run_with_config<F: Fn() + Send + 'static>(
    config: Arc<Config>,
    tx: Sender<Message>,
    rx: Receiver<Message>,
    test_hook: Option<F>,
) -> anyhow::Result<()> {
    let _span = enter_span!(DEBUG, "running");

    debug!("Config: {:?}", config);

    // start responder (processes one event at a time until process terminated)
    let responder_handle = start_responder(rx, &config).context("failed to start responder")?;

    // make sure appropriate paths exist and are reachable
    tx.send(Message::MaybeCreateDirsAll)
        .context("failed to send message")?;

    // init clean
    tx.send(Message::CleanAll)
        .context("failed to send message")?;

    // start all cleaners
    start_cleaners(&tx, &config).context("failed to start cleaners")?;

    // setup all watchers
    start_watchers(&tx, &config).context("failed to setup watchers")?;

    // maybe run test hook (for integration tests)
    test_hook.inspect(|hook_fn| {
        let _span = enter_span!(DEBUG, "test_hook");
        hook_fn();
    });

    // block this thread until the responder thread completes
    responder_handle
        .join()
        .expect("failed to join respender thread")?;

    Ok(())
}
