use std::{
    fs,
    os::unix::fs::symlink,
    path::{Path, PathBuf},
    sync::Arc,
    thread,
    time::Duration,
};

use anyhow::{bail, Context};
use crossbeam_channel::{self as channel, Receiver, Sender};
use notify::{
    event::{ModifyKind, RenameMode},
    Event, EventKind, RecursiveMode, Watcher,
};

mod args;
mod config;

// re-export
pub use args::*;
pub use config::*;
use regex::Regex;
use walkdir::WalkDir;

// TODO:
// - prevent recursive searching when DestDir is within WatchDir or symlinking dirs.

/// Message to be sent throgh the crossbeam_channel.
#[derive(Clone, Debug)]
struct Message {
    rule_idx: usize,
    watch_idx: usize,
    event: Event,
}

macro_rules! match_event_kinds {
    () => {
        EventKind::Modify(ModifyKind::Name(RenameMode::To)) | EventKind::Create(_)
    };
}

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

    // do some init checks and assurances
    init_dirs(&config)?;

    // setup all watchers
    let event_rx = setup_watchers(&config)?;

    // process one event at a time in main until process terminated
    respond_to_messages(event_rx, &config)?;

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

fn is_symlink_valid(path: &Path) -> anyhow::Result<bool> {
    if let Ok(target_path) = fs::read_link(path) {
        if target_path.is_absolute() && fs::metadata(&target_path).is_ok() {
            return Ok(true);
        }
        let dirname = path.parent().unwrap_or_else(|| Path::new(""));
        let resolved = dirname.join(&target_path);
        if fs::metadata(resolved).is_ok() {
            return Ok(true);
        }
    }
    eprintln!("Symlink is broken: {:?}", path);
    Ok(false)
}

fn path_is_rec_subdir_of_any(path: &Path, many_dirs: &[PathBuf]) -> anyhow::Result<bool> {
    Ok(many_dirs.iter().any(|d| path.starts_with(d)))
}

fn init_scan(config: &Arc<Config>) -> anyhow::Result<()> {
    // - walk throgh every dir path recursively with WalkDir...
    // NOTE: BELOW IS LITERALLY just the cleanup function i was wanting to write...
    // TODO: extract all this into a clean-up all dest_dirs function!
    for rule in &config.rules {
        for dest_dir in &rule.dest {
            for entry in WalkDir::new(dest_dir) {
                let entry = entry?;
                let path = entry.path();

                // get file metadata
                let metadata = fs::symlink_metadata(path).with_context(|| {
                    format!(
                        "could not perform metadata call on path or path does not exist: {:?}",
                        path
                    )
                })?;

                // skip this file if not a symlink
                if !metadata.file_type().is_symlink() {
                    eprintln!("This file is not a symlink, skip: {:?}", path);

                    continue;
                }

                // if file doesnt match any regex, it should't belong here... probably...
                if !path_matches_any_regex(path, &rule.regex).context("failed to match regexes")? {
                    eprintln!(
                        "Symlink doesn't match any regex, so deleting symlink i guess: {:?}",
                        path
                    );
                    fs::remove_file(path)?;

                    continue;
                }

                // if symlink is broken, delete!
                if !is_symlink_valid(path).context("failed to check if valid symlink")? {
                    eprintln!("Symlink is broken, so deleting symlink: {:?}", path);
                    fs::remove_file(path)?;

                    continue;
                }

                // if symlink is not a subdir of any watch dir, delete
                if !path_is_rec_subdir_of_any(path, &rule.watch)? {
                    eprintln!(
                        "Symlink is not a subdir of any watch dirs, so deleting symlink: {:?}",
                        path
                    );
                    fs::remove_file(path)?;

                    continue;
                }

                eprintln!("Existing symlink looks good!: {:?}", path);
            }
            eprintln!("cleanup of dest_dir complete!: {:?}", dest_dir);
        }
        eprintln!("cleanup of dest_dirs in rule complete!: {}", rule.name);
    }
    eprintln!("cleanup of all rules complete!");

    // - ensure each is symlink and points to src_path

    // - also need to clean every broken symlink somehow...

    // - create symlinks as needed

    Ok(())
}

fn setup_watchers(config: &Arc<Config>) -> anyhow::Result<Receiver<Message>> {
    let (event_tx, event_rx) = channel::unbounded::<Message>();

    start_watchers_for_each_watch_dir(config, &event_tx)?;

    // disconnect from channel
    drop(event_tx);

    Ok(event_rx)
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
            let tx_clone: Sender<Message> = tx.clone();
            // TODO: instead of simply cloning clone the sender and create a new instance of Message.
            thread::spawn(move || -> anyhow::Result<()> {
                start_watcher(config_arc, rule_idx, watch_idx, tx_clone)
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
            match_event_kinds!() => {
                let new_message = Message {
                    rule_idx,
                    watch_idx,
                    event,
                };
                dbg!(&new_message);
                match tx.send(new_message) {
                    Ok(_) => println!("Watcher sent message!"),
                    Err(e) => println!("WATCHER FAILED TO SEND MESSAGE: {:?}", e),
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

fn respond_to_messages(rx: Receiver<Message>, config: &Arc<Config>) -> anyhow::Result<()> {
    loop {
        match rx.recv() {
            Ok(event) => handle_message(config, &event)?,
            Err(e) => println!("ERROR received from thread: {:?}", e),
        }
    }
}

/// Handle a `notify::Event` received from the crossbeam channel Receiver.
///
/// When a file creation or filename modification `notify::Event` is received,
/// run `handle_path` to check the filename and take action if needed.
fn handle_message(config: &Config, message: &Message) -> anyhow::Result<()> {
    match message.event.kind {
        match_event_kinds!() => {
            for check_path in &message.event.paths {
                handle_path(config, check_path, message).context("failed to handle path")?;
            }
        }
        _ => (),
    }
    Ok(())
}

fn path_matches_any_regex(path: &Path, regexes: &[Regex]) -> anyhow::Result<bool> {
    let filename = path
        .file_name()
        .with_context(|| format!("cannot get OsStr filename of path: {:?}", path))?
        .to_str()
        .with_context(|| format!("cannot convert OsStr to str for path: {:?}", path))?;

    Ok(regexes.iter().any(|r| r.is_match(filename)))
}

fn calc_link_from_src(
    src_path: &Path,
    watch_dir: &Path,
    dest_dir: &Path,
) -> anyhow::Result<PathBuf> {
    let src_path_without_watch_dir = src_path.strip_prefix(watch_dir)?;
    let link = dest_dir.join(src_path_without_watch_dir);

    Ok(link)
}

fn ensure_symlink_and_expected_target(link_path: &Path, src_path: &Path) -> anyhow::Result<()> {
    // something exists here, so ensure that the file at link_path is a symlink
    let is_symlink = fs::symlink_metadata(link_path)?.file_type().is_symlink();
    anyhow::ensure!(
        is_symlink,
        "Error: something already exists at link_path ({:?}) and it's not a symlink?!",
        link_path
    );

    // ensure the existing symlink points to the src_path
    let symlink_points_to_src = src_path == link_path;
    anyhow::ensure!(
        symlink_points_to_src,
        "Error: existing symlink at link_path ({:?}) doesn't point to src_path ({:?})",
        link_path,
        src_path
    );

    Ok(())
}

/// Check if the filename of the path matches the specified Regex's, and take action if needed.
///
/// If it matches, create a symlink to the appropriate dest dir.
fn handle_path(config: &Config, src_path: &Path, message: &Message) -> anyhow::Result<()> {
    let rule = &config.rules[message.rule_idx];
    let watch = &rule.watch[message.watch_idx];

    let regexes = &rule.regex;

    if path_matches_any_regex(src_path, regexes)? {
        eprintln!("Regex matches! {:?}", src_path);

        // For every dest_dir, check if the expected link_path has a symlink, and if not,
        // create one.
        for dest in &rule.dest {
            // ensure that the dest_dir exists
            anyhow::ensure!(
                dest.exists(),
                "Error: dest ({:?}) does not exist... was it deleted?",
                dest
            );

            // where the link_path should be
            let link_path = calc_link_from_src(src_path, watch, dest)?;

            if link_path.exists() {
                // file exists, so now check if it's a symlink and points to src_path
                ensure_symlink_and_expected_target(&link_path, src_path)?;
            } else {
                // file doesn't exist, so create a symlink to there
                symlink(src_path, link_path).context("failed to create symlink")?;
            }
        }
    }

    Ok(())
}
