use std::{fs, sync::Arc, thread, time::Duration};

use anyhow::Context;
use channels::{setup_watchers, start_responder, symlinker::clean_all_dest, Message};

mod args;
mod channels;
mod config;
mod utils;

// re-export
pub use args::*;
pub use config::*;

// TODO:
// - prevent recursive searching when DestDir is within WatchDir or symlinking dirs.

pub fn run() -> anyhow::Result<()> {
    let args = Args {
        config_path: "examples/config.yml".into(),
    };
    run_with_args(args)
}

pub fn run_with_args(args: Args) -> anyhow::Result<()> {
    // create a Config from Args
    let config = Arc::new(Config::new(&args)?);
    dbg!(&config);

    // do some init fs checks and assurances
    init_dirs(&config).context("failed to init dirs")?;

    // create channel
    let (event_tx, event_rx) = crossbeam_channel::unbounded::<Message>();

    // process one event at a time until process terminated
    start_responder(event_rx, &config).context("failed to start responder")?;

    // init scan
    init_scan(&config).context("failed to init scan")?;

    // setup all watchers
    setup_watchers(&config, &event_tx).context("failed to setup watchers")?;

    // disconnect from channel
    drop(event_tx);

    // Keep main alive
    // TODO: implement a proper shutdown procedure (currently killing process)
    loop {
        thread::sleep(Duration::from_secs(1));
    }
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

fn init_scan(config: &Arc<Config>) -> anyhow::Result<()> {
    // ensure that each symlink in dest_dir is valid and appropriate, delete if else
    clean_all_dest(config).context("failed to clean all dest")?;
    // - create symlinks as needed
    // TODO

    Ok(())
}
