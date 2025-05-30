use std::{
    fs,
    path::{Path, PathBuf},
};

use anyhow::Context;
use tempfile::TempDir;

fn create_test_env() -> (TempDir, PathBuf) {
    let temp_dir = TempDir::new().expect("failed to create temp dir");
    let test_path = temp_dir.path().to_path_buf();
    (temp_dir, test_path)
}

fn setup_test_filesystem(root: &Path) -> anyhow::Result<()> {
    fs::create_dir_all(root.join("samples"))?;
    fs::create_dir_all(root.join("favorites"))?;
    fs::write(root.join("samples/sample1.txt"), "")?;
    fs::write(root.join("samples/_sample2.txt"), "")?;

    Ok(())
}

#[test]
fn run_env_test_1() -> anyhow::Result<()> {
    let (temp_dir, test_root) = create_test_env();

    setup_test_filesystem(&test_root).context("failed to setup test filesystem")?;

    // - run main process
    // - start several separate threads:
    //   - perform filesystem operations some seconds later
    //   - terminate this process some more seconds later

    Ok(())
}
