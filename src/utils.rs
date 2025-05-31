use std::{
    fs,
    path::{Path, PathBuf},
};

use anyhow::Context;

#[macro_export]
macro_rules! match_event_kinds {
    () => {
        EventKind::Modify(ModifyKind::Name(RenameMode::To)) | EventKind::Create(_)
    };
}

use regex::Regex;

pub fn is_symlink_valid(path: &Path) -> anyhow::Result<bool> {
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

pub fn calc_dest_link_from_src_orig(
    src_path: &Path,
    watch_dir: &Path,
    dest_dir: &Path,
) -> anyhow::Result<PathBuf> {
    let src_path_without_watch_dir = src_path.strip_prefix(watch_dir)?;
    let link = dest_dir.join(src_path_without_watch_dir);

    Ok(link)
}
