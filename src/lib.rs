use std::{fs, sync::Arc};

use anyhow::Context;
use channels::{cleaning::start_cleaners, start_responder, start_watchers};
use crossbeam_channel::{Receiver, Sender};

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
// - prevent recursive searching when DestDir is within WatchDir or symlinking dirs.

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

    run_with_config(config, tx, rx, None::<fn()>)
}

// TODO: make run_with_args require tx and rx channels
pub fn run_with_config<F: Fn() + Send + 'static>(
    config: Arc<Config>,
    message_tx: Sender<Message>,
    message_rx: Receiver<Message>,
    test_hook: Option<F>,
) -> anyhow::Result<()> {
    dbg!(&config);

    // do some init fs checks and assurances
    init_dirs(&config).context("failed to init dirs")?;

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
        eprintln!("TESTS: RUNNING HOOKS");
        hook();
        eprintln!("TESTS: RAN HOOKS");
    }

    // Block until responder thread completes
    responder_handle
        .join()
        .expect("failed to join respender thread")?;

    Ok(())
}

/// To be run at startup.
/// Initialize directories and catch errors early to prevent mild catastrophes.
fn init_dirs(config: &Config) -> anyhow::Result<()> {
    for rule in &config.rules {
        for path in &rule.watch {
            if path.try_exists()? {
                println!("Path to watch found: {:?}", path);
            } else {
                println!("Path NOT found: {:?}", path);
                if get_setting!(config, rule, create_missing_directories) {
                    println!("Creating directory at: {:?}", path);
                    fs::create_dir_all(path).with_context(|| {
                        format!("failed to create symlink directory: {:?}", path)
                    })?;
                    println!("Created path at: {:?}", path);
                } else {
                    anyhow::bail!("path does not exist! terminating...");
                }
            }
        }
    }
    Ok(())
}

// fn init_scan(config: &Arc<Config>) -> anyhow::Result<()> {
//     // ensure that each symlink in dest_dir is valid and appropriate, delete if else
//     clean_all_dest(config).context("failed to clean all dest")?;
//     // - create symlinks as needed
//     // TODO

//     Ok(())
// }
