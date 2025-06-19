use std::{
    fs::{self, Metadata},
    path::{Path, PathBuf},
    sync::Arc,
};

use anyhow::Context;
use regex::Regex;
use serde::{Serialize, Serializer};
use tracing::debug;

use crate::{Config, Message, Rule};

// generic helpers ////////////////////////////////////////////////////////////

/// A macro to ease the cloning of vars.
#[macro_export]
macro_rules! clone_vars {
    ($($arg:tt),+) => {
        $(
            clone_vars!(@handle $arg);
        )+
    };

    (@handle $var:ident) => {
        let $var = $var.clone();
    };

    (@handle ($var:ident => $new_var:ident)) => {
        let $new_var = $var.clone();
    };

    (@handle ($type:ident :: $var:ident => $new_var:ident)) => {
        let $new_var = $type::clone($var);
    };
}

// Config helpers /////////////////////////////////////////////////////////////

pub type ConfigArc = Arc<Config>;

/// Calculates the number of watch_dirs within Config.
pub fn num_watch_dirs_for_config(config: &ConfigArc) -> anyhow::Result<usize> {
    Ok(config
        .rules
        .iter()
        .map(|r| r.watch_dirs.len())
        .sum::<usize>())
}

// fs helpers /////////////////////////////////////////////////////////////////

/// Returns whether a path is a subdir of any from a list of paths.
pub fn path_is_rec_subdir_of_any(path: &Path, many_dirs: &[PathBuf]) -> anyhow::Result<bool> {
    Ok(many_dirs.iter().any(|d| path.starts_with(d)))
}

/// Returns whether a path filename matches a regex.
pub fn path_matches_any_regex(path: &Path, regexes: &[Regex]) -> anyhow::Result<bool> {
    let raw_basename = path
        .file_name()
        .with_context(|| format!("getting basename: {:?}", path))?;
    let basename = raw_basename
        .to_str()
        .with_context(|| format!("parsing basename to UTF-8: {:?}", raw_basename))?;

    Ok(regexes.iter().any(|r| r.is_match(basename)))
}

/// Returns the target path of a symlink, or None if symlink is broken.
pub fn symlink_target(path: &Path) -> anyhow::Result<Option<PathBuf>> {
    if let Ok(target_path) = fs::read_link(path) {
        if target_path.is_absolute() && fs::metadata(&target_path).is_ok() {
            return Ok(Some(target_path));
        }
        let dirname = path.parent().unwrap_or_else(|| Path::new(""));
        let resolved = dirname.join(&target_path);
        if fs::metadata(resolved).is_ok() {
            return Ok(Some(target_path));
        }
    }
    debug!("Symlink is broken: {:?}", path);
    Ok(None)
}

/// Given an src_dir, calculates a path for where to create a symlink that points to it.
///
/// This is done by stripping the watch_dir prefix off the src_path,
/// and joining it with the link_dir.
///
pub fn calc_link_from_src_orig(
    src_path: &Path,
    watch_dir: &Path,
    link_dir: &Path,
) -> anyhow::Result<PathBuf> {
    let src_path_without_watch_dir = src_path.strip_prefix(watch_dir)?;
    let link = link_dir.join(src_path_without_watch_dir);

    Ok(link)
}

// channel helpers ////////////////////////////////////////////////////////////

/// Sends a shutdown signal to the corresponding Receiver.
pub fn send_shutdown(tx: &crossbeam_channel::Sender<Message>) {
    // shutdown
    tx.send(Message::Shutdown)
        .expect("failed to shutdown, crashing program");
}

// Notify helpers /////////////////////////////////////////////////////////////

/// Matches Notify event kinds on which to perform a filename cookie check on.
#[macro_export]
macro_rules! match_event_kinds {
    () => {
        EventKind::Modify(ModifyKind::Name(RenameMode::To)) | EventKind::Create(_)
    };
}

// logger addons //////////////////////////////////////////////////////////////

/// Creates and enters a new Tracing span.
#[macro_export]
macro_rules! enter_span {
    ($level:ident, $($args:expr)+) => {
        tracing::span!(tracing::Level::$level,
            $(
                $args
            )+
        ).entered()
    };
}

// serde_regex addons /////////////////////////////////////////////////////////

/// A custom serializer for `Option<Vec<Regex>>` since serde_regex doesn't support it.
pub fn custom_serializer_option_vec_regex<S>(
    value: &Option<Vec<Regex>>,
    serializer: S,
) -> Result<S::Ok, S::Error>
where
    S: Serializer,
{
    match value {
        Some(regexes) => {
            let strings: Vec<String> = regexes.iter().map(|r| r.as_str().to_string()).collect();
            strings.serialize(serializer)
        }
        None => serializer.serialize_none(),
    }
}

// to sort ////////////////////////////////////////////////////////////////////

pub fn link_dir_indices(config: &Config) -> impl Iterator<Item = (usize, usize)> + '_ {
    config
        .rules
        .iter()
        .enumerate()
        .flat_map(|(rule_idx, rule)| {
            (0..rule.link_dirs.len()).map(move |link_idx| (rule_idx, link_idx))
        })
}

pub fn link_dir_indices_with_refs(
    config: &Config,
) -> impl Iterator<Item = (usize, usize, &Rule, &PathBuf)> + '_ {
    config
        .rules
        .iter()
        .enumerate()
        .flat_map(|(rule_idx, rule)| {
            rule.link_dirs
                .iter()
                .enumerate()
                .map(move |(link_idx, link_dir)| (rule_idx, link_idx, rule, link_dir))
        })
}

pub fn watch_dir_indices_with_refs(
    config: &Config,
) -> impl Iterator<Item = (usize, usize, &Rule, &PathBuf)> + '_ {
    config
        .rules
        .iter()
        .enumerate()
        .flat_map(|(rule_idx, rule)| {
            rule.watch_dirs
                .iter()
                .enumerate()
                .map(move |(watch_idx, watch_dir)| (rule_idx, watch_idx, rule, watch_dir))
        })
}

pub fn delete_symlink(path: &Path, metadata: &Metadata) -> anyhow::Result<()> {
    if metadata.file_type().is_symlink() {
        fs::remove_file(path)?;
        Ok(())
    } else {
        anyhow::bail!("Not a symlink!!!: {:?}", path)
    }
}
