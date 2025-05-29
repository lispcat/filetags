use std::{fs, path::Path, sync::Arc, thread, time::Duration};

use anyhow::Context;
use args::Args;
use config::Config;
use crossbeam_channel::{self as channel, Sender};
use notify::{
    event::{ModifyKind, RenameMode},
    Event, EventKind, RecursiveMode, Watcher,
};

mod args;
mod config;

// TODO:
// - prevent recursive searching when DestDir is within WatchDir or symlinking dirs.

/// Message to be sent throgh the crossbeam_channel.
#[derive(Clone)]
pub struct Message {
    rule_idx: usize,
    watch_idx: usize,
    event: Event,
}

macro_rules! event_kinds_to_match {
    () => {
        EventKind::Modify(ModifyKind::Name(RenameMode::To)) | EventKind::Create(_)
    };
}

fn main() -> anyhow::Result<()> {
    // init ///////////////////////////////////////////////////////////////////

    let args = Args {
        config_path: "examples/config.yml".into(),
    };

    let config = Arc::new(Config::new(&args)?);
    eprintln!("CONFIG: {:#?}", config);

    init_dirs(&config)?;

    // TODO: first scan ///////////////////////////////////////////////////////
    // - delete all broken symlinks in dest_dir (later, run this every config.clean_interval).
    // - scan all files recursively under watch_dir, create symlinks as appropriate.
    //   - if symlink already exists at Dest,
    //     - if points to orig file, do nothing.
    //     - if points to another file, TODO process error appropriately.

    // set up watchers ////////////////////////////////////////////////////////

    let (event_tx, event_rx) = channel::unbounded::<Message>();

    start_watchers_for_each_watch_dir(&config, &event_tx)?;

    // disconnect from channel in main
    drop(event_tx);

    // process one event at a time in main until process terminated
    loop {
        match event_rx.recv() {
            Ok(event) => handle_message(&config, &event)?,
            Err(e) => println!("ERROR received from thread: {:?}", e),
        }
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

/// For each watch dir, spawn a notify watcher, where every `notify::Event` the watcher creates
/// is forwarded to its corresponding crossbeam channel Receiver from the calling function.
///
/// Each watcher is physically started by running the `start_watcher` function.
fn start_watchers_for_each_watch_dir(
    config: &Arc<Config>,
    tx: &Sender<Message>,
) -> anyhow::Result<()> {
    // start watcher for each watch_dir
    for (rule_idx, rule) in config.rules.iter().enumerate() {
        for (watch_idx, _) in rule.watch.iter().enumerate() {
            let config_arc = Arc::clone(config);
            let tx: Sender<Message> = tx.clone();
            thread::spawn(move || -> anyhow::Result<()> {
                start_watcher(config_arc, rule_idx, watch_idx, tx)
            });
        }
    }
    Ok(())
}

/// Starts a notify watcher, where every `notify::Event` the watcher creates is forwarded
/// to its corresponding crossbeam channel Receiver.
///
/// This function has a never-ending loop at the end to keep the watcher alive.
/// This function is meant to be ran as a new thread, specifically in the function
/// `start_watchers_for_each_watch_dir`.
fn start_watcher(
    config: Arc<Config>,
    rule_idx: usize,
    watch_idx: usize,
    tx: Sender<Message>,
) -> anyhow::Result<()> {
    let rule = &config.rules[rule_idx];
    let watch = &rule.watch[watch_idx];

    let mut watcher = notify::recommended_watcher(move |res: Result<Event, _>| match res {
        Ok(event) => match event.kind {
            // only send message if filename modification or file creation
            event_kinds_to_match!() => {
                let new_message = Message {
                    rule_idx,
                    watch_idx,
                    event,
                };
                match tx.send(new_message) {
                    Ok(x) => println!("SENT!!!: {:?}", x),
                    Err(e) => println!("FAILED TO SEND: {:?}", e),
                }
            }
            // for all other events do nothing
            _ => (),
        },
        Err(e) => {
            println!("Watch Error! {}", e);
        }
    })?;

    println!("Starting watcher at: {:?}", watch);
    watcher.watch(watch, RecursiveMode::Recursive)?;

    // Keep the watcher alive - it will send events via the closure
    loop {
        thread::sleep(Duration::from_secs(1));
    }
}

/// Handle a `notify::Event` received from the crossbeam channel Receiver.
///
/// When a file creation or filename modification `notify::Event` is received,
/// run `handle_path` to check the filename and take action if needed.
fn handle_message(config: &Config, message: &Message) -> anyhow::Result<()> {
    let event = &message.event;
    match event.kind {
        event_kinds_to_match!() => {
            for path in &event.paths {
                handle_path(config, path, message).context("failed to handle path")?;
            }
        }
        _ => (),
    }
    Ok(())
}

// !
// TODO: does each Event need config and rule information? ////////////////////
//   if so, then each Event should be an enum containing an additional rule_idx and watch_idx.
// !

/// Check if the filename of the path matches the specified Regex's, and take action if needed.
///
/// If it matches, create a symlink to the appropriate dest dir.
fn handle_path(config: &Config, path: &Path, message: &Message) -> anyhow::Result<()> {
    println!("DEBUG: handle path: {:?}", path);
    //     let filename = path
    //         .file_name()
    //         .with_context(|| format!("cannot get OsStr filename of path: {:?}", path))?
    //         .to_str()
    //         .with_context(|| format!("cannot convert OsStr to str for path: {:?}", path))?;

    //     if config.pattern.is_match(filename) {
    //         eprintln!("REGEX MATCHES!: {}", filename);
    //     } else {
    //         eprintln!("Regex does not match: {}", filename);
    //     }
    Ok(())
}

// /// Creates a symlink from orig_path to dest_dir.
// fn create_symlink(orig_path: &Path, dest_dir: &Path) -> anyhow::Result<()> {
//     eprintln!(
//         "Creating symlink from ({:?}) to ({:?})...",
//         orig_path, dest_dir
//     );
//     Ok(())
// }
