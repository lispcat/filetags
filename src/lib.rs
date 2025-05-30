use std::{
    fs,
    os::unix::fs::symlink,
    path::{Path, PathBuf},
    sync::Arc,
    thread,
    time::Duration,
};

use anyhow::Context;
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

fn init_scan(config: &Arc<Config>) -> anyhow::Result<()> {
    // - walk throgh every dir path recursively with WalkDir...

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

fn filename_matches_any_regex(filename: &str, regexes: &[Regex]) -> anyhow::Result<bool> {
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
    let filename = src_path
        .file_name()
        .with_context(|| format!("cannot get OsStr filename of path: {:?}", src_path))?
        .to_str()
        .with_context(|| format!("cannot convert OsStr to str for path: {:?}", src_path))?;

    let rule = &config.rules[message.rule_idx];
    let watch = &rule.watch[message.watch_idx];

    let regexes = &rule.regex;

    if filename_matches_any_regex(filename, regexes)? {
        eprintln!("Regex matches! {:?}", filename);

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
