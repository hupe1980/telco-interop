//! The command line prints `telco_interop::report`'s lines: one rendering path for the binary
//! and the site's playground.
#![cfg(feature = "cli")]
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::indexing_slicing)]

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use telco_interop::report;

fn run<S: AsRef<std::ffi::OsStr>>(args: &[S]) -> (String, i32) {
    let o = Command::new(env!("CARGO_BIN_EXE_telco-interop")).args(args).output().unwrap();
    (String::from_utf8(o.stdout).unwrap(), o.status.code().unwrap())
}

fn all_fixtures() -> Vec<PathBuf> {
    let mut v = Vec::new();
    for interface in ["wbci", "spri"] {
        let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../").join(interface).join("tests/fixtures");
        let mut here: Vec<PathBuf> = fs::read_dir(dir).unwrap().map(|e| e.unwrap().path()).collect();
        here.sort();
        v.extend(here);
    }
    v
}

/// What the binary prints for one file: its path, then the lines indented by two spaces.
fn expected(path: &Path, out: &report::Output) -> String {
    let mut s = format!("{}\n", path.display());
    for line in &out.lines {
        s.push_str("  ");
        s.push_str(line);
        s.push('\n');
    }
    s
}

#[test]
fn read_and_check_print_the_report_lines_for_every_fixture() {
    // playground/SR-005
    let files = all_fixtures();
    assert_eq!(files.len(), 45);
    for file in &files {
        let bytes = fs::read(file).unwrap();
        for (cmd, out) in [("read", report::read(&bytes)), ("check", report::check(&bytes))] {
            let (printed, code) = run(&[Path::new(cmd), file]);
            assert_eq!(printed, expected(file, &out), "{cmd} {}", file.display());
            assert_eq!(code == 0, out.ok, "{cmd} {}", file.display());
        }
    }
}

#[test]
fn an_unreadable_payload_prints_the_report_line() {
    // playground/SR-005
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join("report");
    fs::create_dir_all(&dir).unwrap();
    let file = dir.join("doctype.xml");
    fs::write(&file, "<!DOCTYPE x><x/>").unwrap();
    let bytes = fs::read(&file).unwrap();
    for (cmd, out) in [("read", report::read(&bytes)), ("check", report::check(&bytes))] {
        let (printed, code) = run(&[Path::new(cmd), &file]);
        assert_eq!(printed, expected(&file, &out), "{cmd}");
        assert_eq!(code, 1);
        assert!(!out.ok);
    }
}

#[test]
fn code_prints_the_report_lines() {
    // playground/SR-005
    for (args, wbci, spri) in [
        (&["code", "8103"][..], true, true),
        (&["code", "0021", "--spri"][..], false, true),
        (&["code", "OK"][..], true, true),
        (&["code", "0021", "--wbci"][..], true, false),
    ] {
        let out = report::code(args[1], wbci, spri);
        let (printed, code) = run(args);
        assert_eq!(printed, format!("{out}\n"), "{args:?}");
        assert_eq!(code == 0, out.ok, "{args:?}");
    }
}
