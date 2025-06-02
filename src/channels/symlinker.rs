use std::{fs, os::unix::fs::symlink, path::Path, sync::Arc};

use anyhow::Context;
use notify::{
    event::{ModifyKind, RenameMode},
    EventKind,
};
use walkdir::WalkDir;

use crate::{
    match_event_kinds,
    utils::{
        calc_dest_link_from_src_orig, is_symlink_valid, path_is_rec_subdir_of_any,
        path_matches_any_regex,
    },
    Config,
};

use super::WatchEvent;

/// Handle a `notify::Event` received from the crossbeam channel Receiver.
///
/// When a file creation or filename modification `notify::Event` is received,
/// run `handle_path` to check the filename and take action if needed.
pub fn handle_event_message(config: &Config, message: &WatchEvent) -> anyhow::Result<()> {
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
fn handle_path(config: &Config, src_path: &Path, message: &WatchEvent) -> anyhow::Result<()> {
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
            let link_path = calc_dest_link_from_src_orig(src_path, watch, dest)?;

            if link_path.exists() {
                // file exists, so now check if it's a symlink and points to src_path
                ensure_is_symlink_and_expected_target(&link_path, src_path)?;
            } else {
                // file doesn't exist, so create a symlink to there
                symlink(src_path, link_path).context("failed to create symlink")?;
            }
        }
    }

    Ok(())
}

pub fn clean_all_dest(config: &Arc<Config>) -> anyhow::Result<()> {
    // - walk throgh every dir path recursively with WalkDir...
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

    Ok(())
}

pub fn ensure_is_symlink_and_expected_target(
    link_path: &Path,
    src_path: &Path,
) -> anyhow::Result<()> {
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
