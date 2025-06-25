use std::sync::Arc;

use clap::Parser;
use systemd::daemon;
use tracing::debug;

// modules
mod args;
mod config;
mod dispatch;
mod logger;
mod utils;

// re-export
pub use args::*;
pub use config::*;
pub use dispatch::*;
pub use logger::*;
pub use utils::*;

use crate::{actions::Action, workers::WorkerType};

// TODO:
// - prevent recursive searching when LinkDir is within WatchDir or symlinking dirs.

/// The default run command.
pub async fn run() -> anyhow::Result<()> {
    let args = Args::parse();
    let (tx, rx) = tokio::sync::mpsc::unbounded_channel::<Message>();
    run_with_args(args, tx, rx).await
}

/// Run the program with args, tx, and rx.
pub async fn run_with_args(
    args: Args,
    tx: Sender<Message>,
    rx: Receiver<Message>,
) -> anyhow::Result<()> {
    // create a Config from Args
    let config: Arc<Config> = Config::create(&args)?;
    let _logger = Logger::new();

    run_with_config(config, tx, rx, None::<fn()>).await
}

/// Run the program with config, tx, rx, and optionally test_hook.
pub async fn run_with_config<F>(
    config: Arc<Config>,
    tx: Sender<Message>,
    rx: Receiver<Message>,
    test_hook: Option<F>,
) -> anyhow::Result<()>
where
    F: Fn() + Send + 'static,
{
    span_enter!(DEBUG, "running");
    debug!("Config: {:#?}", config);

    // start responder
    let dispatcher = Dispatcher::new(rx, tx, Arc::clone(&config))?
        // create all necessary dirs
        .run(Action::MakeNecessaryDirs)?
        // clean all broken or innapropriate links in link_dirs
        .run(Action::CleanAll)?
        // maybe create symlinks as appropriate
        .run(Action::SymlinkAll)?
        // start all link cleaners
        .launch(WorkerType::Cleaners)?
        // setup all watchers
        .launch(WorkerType::Watchers)?;

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
    dispatcher
        .rx_handle
        .await
        .expect("failed to await responder thread")?;
    // Handle::current()
    //     .block_on(dispatcher.rx_handle)
    //     .expect("failed to join respender thread")?;

    Ok(())
}
