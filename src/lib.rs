use std::{fs, sync::Arc};

use anyhow::Context;
use channels::{
    actions::cleaning::start_cleaners, responder::start_responder, watcher::start_watchers,
};
use crossbeam_channel::{Receiver, Sender};
use tracing::{debug, info_span, span};

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
    let args = Args {
        config_path: "examples/config.yml".into(),
    };

    let (tx, rx) = crossbeam_channel::unbounded::<Message>();

    run_with_args(args, tx, rx)
}

pub fn run_with_args(args: Args, tx: Sender<Message>, rx: Receiver<Message>) -> anyhow::Result<()> {
    // create a Config from Args
    let config = Arc::new(Config::new(&args)?);
    let _logger = Logger::new();

    let _span_guard = info_span!("Run").entered();
    run_with_config(config, tx, rx, None::<fn()>)
}

// TODO: make run_with_args require tx and rx channels
pub fn run_with_config<F: Fn() + Send + 'static>(
    config: Arc<Config>,
    message_tx: Sender<Message>,
    message_rx: Receiver<Message>,
    test_hook: Option<F>,
) -> anyhow::Result<()> {
    debug!("The Config: {:?}", config);

    // do some init fs checks and assurances
    init_fs(&config).context("failed to init fs")?;

    // process one event at a time until process terminated
    let responder_handle =
        start_responder(message_rx, &config).context("failed to start responder")?;

    // init clean
    message_tx
        .send(Message::CleanAll)
        .context("failed to send message")?;

    // start all cleaners
    start_cleaners(&message_tx, &config).context("failed to start cleaners")?;

    // setup all watchers
    start_watchers(&message_tx, &config).context("failed to setup watchers")?;

    // maybe run test hook
    if let Some(hook) = test_hook {
        let _span = enter_span!(DEBUG, "test_hook");
        hook();
    }

    // Block until responder thread completes
    responder_handle
        .join()
        .expect("failed to join respender thread")?;

    Ok(())
}

/// To be run at startup.
/// Initialize directories and catch errors early to prevent mild catastrophes.
fn init_fs(config: &Config) -> anyhow::Result<()> {
    let _span = enter_span!(INFO, "init_dirs");

    for rule in &config.rules {
        for path in &rule.watch_dirs {
            if path.try_exists()? {
                debug!(?path, "Path to watch found");
            } else {
                debug!(?path, "Path to watch not found");
                if rule.settings.create_missing_directories {
                    debug!(?path, "Creating directory");
                    fs::create_dir_all(path).with_context(|| {
                        format!("failed to create symlink directory: {:?}", path)
                    })?;
                } else {
                    anyhow::bail!("Path does not exist! terminating...");
                }
            }
        }
    }
    Ok(())
}

// fn init_scan(config: &Arc<Config>) -> anyhow::Result<()> {
//     // ensure that each symlink in link_dir is valid and appropriate, delete if else
//     clean_all_dest(config).context("failed to clean all dest")?;
//     // - create symlinks as needed
//     // TODO

//     Ok(())
// }
