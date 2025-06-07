mod common;

use std::{fs, sync::Arc, thread, time::Duration};

use filetags::{run_with_config, send_shutdown, Config, Logger, Message, Rule};
use regex::Regex;

use common::*;
use tracing::{info, warn};

#[test]
fn logging_wip() {
    let logger = Logger::new();

    info!("This is an info message!");
    warn!("This is a warn message!");
}

#[test]
fn basic1() {
    // init
    let (_, root) = create_test_env();
    let (tx, rx) = create_tx_rx!();

    // create dirs
    let_paths!(
        (watch_dir = root / "watch_dir" : create = "dir"),
        (dest_dir = root / "dest_dir"   : create = "dir"),
    );

    // create files
    let_paths!(
        // expect no action
        (file1 = watch_dir / "file1.txt"  : create = "f"),

        // expect init scan to symlink
        (file2 = watch_dir / "_file2.txt" : create = "f"),

        // test hook will rename
        (file3 = watch_dir / "file3.txt"          : create = "f"),
        (file3_renamed = watch_dir / "_file3.txt" : create = "no"),

        // // test hook will create
        (file4 = watch_dir / "_file4.txt" : create = "no")
    );

    // define config
    let config = create_config!(("test", (watch_dir), (dest_dir), "^_.*"));

    let test_hook = {
        clone_vars!(tx, file3, file3_renamed, file4);
        move || {
            rename_file(&file3, &file3_renamed);
            fs::File::create(&file4).expect("failed to create files");

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
            "dest_dir",
            "dest_dir/_file2.txt",
            "dest_dir/_file3.txt",
            "dest_dir/_file4.txt",
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

    // create dirs
    let_paths!(
        (watch_dir = root / "watch_dir" : create = "dir"),
        (dest_dir = root / "dest_dir"   : create = "dir"),
    );

    // create files
    let_paths!(
        // expect no action, because symlink already created
        (file1 = watch_dir / "_file1.txt"        : create = "f"),
        (file1_symlink = dest_dir / "_file1.txt" : create = "symlink" => file1),

        // broken symlink, delete at init
        (file2_non_existent = watch_dir / "_file2.txt"  : create = "no"),
        (file2_broken_symlink = dest_dir / "_file2.txt" : create = "symlink" => file2_non_existent),
    );

    // define config
    let config = create_config!(("test", (watch_dir), (dest_dir), "^_.*"));

    // define test hook
    let test_hook = {
        clone_vars!(tx);
        move || {
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
            "dest_dir",
            "dest_dir/_file1.txt",
            "watch_dir",
            "watch_dir/_file1.txt",
        ],
    );
}

// #[test]
// fn run_env_test_1() -> anyhow::Result<()> {
//     // init
//     let (_, root) = create_test_env();
//     let (tx, rx) = create_tx_rx!();

//     // create dirs
//     let_paths!(
//         (watch_dir = root / "watch_dir" : create = "dir"),
//         (dest_dir = root / "dest_dir"   : create = "dir"),
//     );

//     // create files
//     let_paths!(
//         // // expect no action
//         // (file__untouched = watch_dir / "file1.txt"    : create = "f"),

//         // // expect init scan to symlink
//         // (file__init_scan = watch_dir / "_file2.txt" : create = "f"),

//         // expect test hook to symlink
//         (file_r = watch_dir / "file3.txt"   : create = "f"),
//         (file_rn = watch_dir / "_file3.txt" : create = "no"),

//         // // expect init scan to take no action
//         // // because already symlinked
//         // (file4 = watch_dir / "_file4.txt"         : create = "f"),
//         // (file4_symlink = dest_dir / "_file4.txt"  : create = "symlink" => file4),
//         // // expect init scan to delete symlink
//         // // since broken
//         // (file5_no_file = watch_dir / "file5.txt"        : create = "no"),
//         // (file5_broken_symlink = dest_dir / "_file5.txt" : create = "symlink" => file5_no_file),
//     );

//     // define config
//     let config = create_config!(("test", (watch_dir), (dest_dir), "^_.*"));

//     // test hook
//     set_test_hook({
//         fn rename(tx: &crossbeam_channel::Sender<Message>, file3: &Path, file3_renamed: &Path) {
//             // rename file3 to file3_renamed
//             fs::rename(file3, file3_renamed).expect("failed to rename file");

//             // shutdown
//             thread::sleep(Duration::from_millis(100));
//             tx.clone()
//                 .send(Message::Shutdown)
//                 .expect("failed to shutdown");
//         }
//         // TODO: create macro for cloning args first
//         let tx_copy = tx.clone();
//         let file_r_copy = file_r.clone();
//         let file_rn_copy = file_rn.clone();
//         move || rename(&tx_copy, &file_r_copy, &file_rn_copy)
//     });

//     // start the main process loop
//     run_with_config(config, tx, rx)?;

//     // print all files recursively
//     WalkDir::new(root)
//         .into_iter()
//         .for_each(|e| println!("{}", e.unwrap().path().display()));

//     // verify fs

//     // // file1
//     // assert!(file_control.exists());

//     // // file2
//     // assert!(file_init_scan.exists());
//     // assert!(dest_dir.join(file_init_scan.file_name().unwrap()).exists());

//     // // file_r
//     // let file_rn_symlink = dest_dir.join(file_rn.file_name().unwrap());
//     // assert!(file_rn_symlink.exists());

//     Ok(())
// }
