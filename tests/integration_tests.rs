mod common;

use std::{fs, sync::Arc, thread, time::Duration};

use filetags::{clone_vars, run_with_config, send_shutdown, Config, Logger, Message, Rule};
use regex::Regex;

use common::*;
use tracing::{info, info_span, warn};

#[test]
fn logging_wip() {
    let _logger = Logger::new();

    info!("This is an info message!");
    warn!("This is a warn message!");
}

#[test]
fn basic1() {
    // init
    let (_, root) = create_test_env();
    let (tx, rx) = create_tx_rx!();
    let _logger = Logger::new();
    let _span = info_span!("T_basic1").entered();

    // create dirs
    let_paths!(
        (watch_dir = root / "watch_dir" : create = "dir"),
        (link_dir = root / "link_dir"   : create = "dir"),
    );

    // create files
    let_paths!(
        // not a match, take no action
        (file1 = watch_dir / "file1.txt"  : create = "f"),

        // is a match, expect the init scan to create symlink
        (file2 = watch_dir / "_file2.txt" : create = "f"),

        // the test_hook will rename with underscore, expect Notify to create symlink
        (file3 = watch_dir / "file3.txt"          : create = "f"),
        (file3_renamed = watch_dir / "_file3.txt" : create = "no"),

        // the test_hook will create this file, expect Notify to create symlink
        (file4 = watch_dir / "_file4.txt" : create = "no")
    );

    // define config
    let config = create_config!(("test", (watch_dir), (link_dir), "^_.*"));

    let test_hook = {
        clone_vars!(tx, file3, file3_renamed, file4);
        move || {
            rename_file(&file3, &file3_renamed);
            fs::File::create(&file4).expect("failed to create files");

            // TODO: have it check for corresponding logs instead of waiting 1/10 a sec
            thread::sleep(Duration::from_millis(100));
            send_shutdown(&tx);
        }
    };

    // start the main process loop
    run_with_config(config, tx, rx, Some(test_hook)).expect("failed to run main");

    // assertions
    assert_cur_and_exp_trees_eq(
        &root,
        vec![
            "link_dir",
            "link_dir/_file2.txt",
            "link_dir/_file3.txt",
            "link_dir/_file4.txt",
            "watch_dir",
            "watch_dir/_file2.txt",
            "watch_dir/_file3.txt",
            "watch_dir/_file4.txt",
            "watch_dir/file1.txt",
        ],
    );
}

// TODO: make this into a bunch of tinier integ tests, where there's only one file per test (use a macro for templating this whole thing!!!!! and make like, 10 basic tests!)

#[test]
fn basic2() {
    // init
    let (_, root) = create_test_env();
    let (tx, rx) = create_tx_rx!();
    let _logger = Logger::new();
    let _span = info_span!("T_basic2").entered();

    // create dirs
    let_paths!(
        (watch_dir = root / "watch_dir" : create = "dir"),
        (link_dir = root / "link_dir"   : create = "dir"),
    );

    // create files
    let_paths!(
        // symlink already created, expect no action
        (file1 = watch_dir / "_file1.txt"        : create = "f"),
        (file1_symlink = link_dir / "_file1.txt" : create = "symlink" -> file1),

        // broken symlink, delete at startup
        (file2_non_existent = watch_dir / "_file2.txt"  : create = "no"),
        (file2_broken_symlink = link_dir / "_file2.txt" : create = "symlink" -> file2_non_existent),
    );

    // define config
    let config = create_config!(("test", (watch_dir), (link_dir), "^_.*"));

    // define test hook
    let test_hook = {
        clone_vars!(tx);
        move || {
            // TODO: have it check for corresponding logs instead of waiting 1/10 a sec
            thread::sleep(Duration::from_millis(100));
            send_shutdown(&tx);
        }
    };

    // start the main process loop
    run_with_config(config, tx, rx, Some(test_hook)).expect("failed to run main");

    // assertions
    assert_cur_and_exp_trees_eq(
        &root,
        vec![
            "link_dir",
            "link_dir/_file1.txt",
            "watch_dir",
            "watch_dir/_file1.txt",
        ],
    );
}
