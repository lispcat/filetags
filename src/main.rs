use std::{fs, os::unix::fs::symlink, path::Path, sync::Arc, thread, time::Duration};

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
#[derive(Clone, Debug)]
pub struct Message {
    rule_idx: usize,
    watch_idx: usize,
    event: Event,
}

macro_rules! match_event_kinds {
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
    dbg!(&config);

    init_dirs(&config)?;

    // TODO: init_scan ///////////////////////////////////////////////////////
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

    let is_match: bool = regexes.iter().any(|r| r.is_match(filename));
    if is_match {
        eprintln!("Regex matches! {:?}", filename);
        eprintln!("Starting symlink creation...");

        for dest in &rule.dest {
            if !dest.exists() {
                // this should not happen because init_dirs shouldve covered this
                anyhow::bail!(format!(
                    "Error: dest ({:?}) does not exist... was it deleted?",
                    dest
                ));
            } else {
                let trailing_src_path = src_path.strip_prefix(watch)?;
                let link_path = dest.join(trailing_src_path);

                // now that the new link to create has been identified,
                // should a new link be created? is a link already there? etc...

                if !link_path.exists() {
                    // link path doesn't exist, so create a symlink here.
                    symlink(src_path, link_path).context("failed to create symlink")?;
                } else {
                    // something already exists here... is it what's supposed to be here?a
                    let is_symlink = fs::symlink_metadata(src_path)?.file_type().is_symlink();

                    if !is_symlink {
                        // something exists at link_path but it's not a symlink?!?!
                        anyhow::bail!(
                            format!(
                                "Error: something already exists at link_path ({:?}) and it's not a symlink?!",
                                link_path
                            )
                        );
                    } else {
                        // it's a symlink! but does it point to the right src_path?
                        let symlink_points_to_src = src_path == link_path;

                        if !symlink_points_to_src {
                            // an incorrect symlink points here... this is problematic...
                            anyhow::bail!(
                                format!(
                                    "Error: existing symlink at link_path ({:?}) doesn't point to src_path ({:?})",
                                    link_path,
                                    src_path
                                )
                            );
                        } else {
                            // symlink exists and it points to the correct src_path!
                            // so no further actions needed
                        }
                    }
                }
            }
        }

        // TODO:
        // - since the regex matches, there expects a symlink to the file in the rule's dest dirs.
        // - check if the symlinks already exists in each dest dirs.
        //   - if yes, continue
        //   - if no, create a new symlink
    }

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
