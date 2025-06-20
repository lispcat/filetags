use std::{
    fs::{self, Metadata},
    os::unix::fs::symlink,
    path::Path,
    sync::Arc,
};

use anyhow::Context;
use crossbeam_channel::Sender;
use notify::{
    event::{ModifyKind, RenameMode},
    EventKind,
};
use tracing::debug;
use walkdir::WalkDir;

use crate::{
    delete_symlink, get_basename, match_event_kinds, symlink_target,
    utils::{calc_link_from_src_orig, path_matches_any_regex},
    watch_dir_indices_with_refs, Config, Message, WatchEvent,
};

/// Shorthand for sending a query to the Receiver to symlink_create_all.
pub fn query_symlink_create_all(tx: &Sender<Message>) -> anyhow::Result<()> {
    tx.send(Message::SymlinkCreateAll)
        .context("sending message")
}

/// Runs `symlink_create` for every watch_dir in Config.
pub fn symlink_create_all(config: &Arc<Config>) -> anyhow::Result<()> {
    for (rule_idx, watch_idx, _, watch_dir) in watch_dir_indices_with_refs(config) {
        for direntry in WalkDir::new(watch_dir) {
            symlink_create(config, direntry.unwrap().path(), rule_idx, watch_idx)?;
        }
    }

    Ok(())
}
/// Handle a notify event.
/// Called from the Receiver.
///
/// Runs `maybe_symlink_path` if the notify event matches `match_event_kinds!()`.
pub fn handle_notify_event(config: &Config, message: &WatchEvent) -> anyhow::Result<()> {
    match message.event.kind {
        match_event_kinds!() => {
            for check_path in &message.event.paths {
                symlink_create(config, check_path, message.rule_idx, message.watch_idx)
                    .context("handling path for notify event")?;
            }
        }
        _ => (),
    }
    Ok(())
}

/// Maybe create a symlink to the given path.
///
/// First it checks if it matches any of the regexes. If it matches, then create a symlink
/// if not already created.
pub fn symlink_create(
    config: &Config,
    src_path: &Path,
    rule_idx: usize,
    watch_idx: usize,
) -> anyhow::Result<()> {
    let rule = &config.rules[rule_idx];
    let watch = &rule.watch_dirs[watch_idx];
    let regexes = &rule.regex;

    if path_matches_any_regex(src_path, regexes)? {
        debug!("Regex matches! {:?}", src_path);

        // For every link_dir, check if the expected link_path has a symlink, and if not,
        // create one.
        for link in &rule.link_dirs {
            // error if the link_dir doesn't exist
            anyhow::ensure!(
                link.exists(),
                "link ({:?}) does not exist... was it deleted?",
                link
            );

            // where the symlink_path should be
            let symlink_path = calc_link_from_src_orig(src_path, watch, link)?;

            // try symlinking
            try_symlinking(&symlink_path, src_path)?;
        }
    }

    Ok(())
}

/// Try creating a symlink at symlink_path to src_path.
///
/// If a symlink already exists at symlink_path, `validate_existing_symlink`.
/// Otherwise, create a symlink.
fn try_symlinking(symlink_path: &Path, src_path: &Path) -> anyhow::Result<()> {
    // check if a file exists here
    if symlink_path.exists() {
        // check if that file is a symlink
        let metadata = fs::symlink_metadata(symlink_path)?;
        if metadata.file_type().is_symlink() {
            // a symlink already exists here. we expect it to point to the src_path...
            // but what if it doesn't?
            validate_existing_symlink(symlink_path, src_path, &metadata)?;
        } else {
            anyhow::bail!(
                "failed to create symlink at {:?} with target {:?}. a non-symlink file already exists at symlink path.",
                symlink_path,
                src_path
            )
        }
    } else {
        // file doesn't exist, so create a symlink to there
        symlink(src_path, symlink_path).context("creating symlink")?;
    }

    Ok(())
}

/// Validate that the existing symlink works and points to the correct target.
///
/// If the symlink is broken, delete it.
/// If it doesn't point to the correct target, modify the filename slightly and try again by
/// running `try_symlinking` (recursive).
pub fn validate_existing_symlink(
    symlink_path: &Path,
    src_path: &Path,
    metadata: &Metadata,
) -> anyhow::Result<()> {
    match symlink_target(symlink_path)? {
        // symlink is broken
        None => {
            debug!("Symlink is broken, deleting symlink: {:?}", symlink_path);
            delete_symlink(symlink_path, metadata)?;
        }
        // symlink target exists
        Some(symlink_target) => {
            // check if src_path points to target
            if src_path == symlink_target {
                // src_path does indeed point to target!
                debug!(
                    "Symlink points to the correct source file! {:?}, {:?}, {:?}",
                    src_path, symlink_target, symlink_path
                );
            } else {
                // doesn't point to target...
                debug!(
                    "symlink at link_path {:?} doesn't point to src_path {:?}",
                    symlink_path, src_path
                );
                // rename by prepending with "0_"
                let renamed_symlink_path =
                    symlink_target.with_file_name(format!("0_{}", get_basename(symlink_path)?));
                debug!(
                    "renamed symlink path {:?} to {:?}",
                    symlink_path, renamed_symlink_path
                );

                // try symlinking again... (recursive)
                try_symlinking(&renamed_symlink_path, src_path)?;
            }
        }
    };

    Ok(())
}
