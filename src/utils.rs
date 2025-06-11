use std::{
    fs,
    path::{Path, PathBuf},
    sync::Arc,
};

use anyhow::Context;

#[macro_export]
macro_rules! match_event_kinds {
    () => {
        EventKind::Modify(ModifyKind::Name(RenameMode::To)) | EventKind::Create(_)
    };
}

use regex::Regex;
use tracing::debug;

use crate::{Config, Message};

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

pub fn path_is_rec_subdir_of_any(path: &Path, many_dirs: &[PathBuf]) -> anyhow::Result<bool> {
    Ok(many_dirs.iter().any(|d| path.starts_with(d)))
}

pub fn path_matches_any_regex(path: &Path, regexes: &[Regex]) -> anyhow::Result<bool> {
    let filename = path
        .file_name()
        .with_context(|| format!("cannot get OsStr filename of path: {:?}", path))?
        .to_str()
        .with_context(|| format!("cannot convert OsStr to str for path: {:?}", path))?;

    Ok(regexes.iter().any(|r| r.is_match(filename)))
}

pub fn calc_link_from_src_orig(
    src_path: &Path,
    watch_dir: &Path,
    link_dir: &Path,
) -> anyhow::Result<PathBuf> {
    let src_path_without_watch_dir = src_path.strip_prefix(watch_dir)?;
    let link = link_dir.join(src_path_without_watch_dir);

    Ok(link)
}

pub fn send_shutdown(tx: &crossbeam_channel::Sender<Message>) {
    // shutdown
    tx.send(Message::Shutdown)
        .expect("failed to shutdown, crashing program");
}

pub fn num_watch_dirs_for_config(config: &Arc<Config>) -> anyhow::Result<usize> {
    let val: u128 = (config
        .rules
        .iter()
        .map(|r| r.watch_dirs.len())
        .sum::<usize>()
        + 1)
    .try_into()
    .expect("failed to convert usize to u128");

    if val > usize::MAX as u128 {
        anyhow::bail!(
            "number of watch_dirs ({:?}) exceeds usize::MAX ({:?}). this is probably a bug, unless your config file is that massive.",
            val,
            usize::MAX
        );
    } else {
        Ok(val as usize)
    }
}

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

#[macro_export]
macro_rules! clone_vars {
    ($($var:ident),+) => {
        $(
            let $var = $var.clone();
        )+
    };
}
