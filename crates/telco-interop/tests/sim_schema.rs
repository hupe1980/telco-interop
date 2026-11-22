//! Conformance: every payload of every scenario validates against the AK S&P's WBCI XML schema
//! V2.00 and S/PRI schema 4.00.03, fetched by `scripts/fetch-wbci-xsd.sh` and
//! `scripts/fetch-spri-xsd.sh` and never committed.
//!
//! Without a schema or `xmllint` the test reports itself skipped; `WBCI_XSD_REQUIRED=1` and
//! `SPRI_XSD_REQUIRED=1` (set by `just schema`) turn a skip into a failure.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::indexing_slicing)]

use std::path::{Path, PathBuf};
use std::process::Command;

use telco_interop::sim::{self, Inhalt};

fn workspace() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

/// The schema entry point, or `None` (skipped); a failure when the job requires it.
fn schema(var_dir: &str, default: &str, entry: &str, var_required: &str) -> Option<PathBuf> {
    let dir = std::env::var_os(var_dir).map_or_else(|| workspace().join(default), PathBuf::from);
    let required = std::env::var(var_required).is_ok_and(|v| v == "1");
    let xmllint = Command::new("xmllint").arg("--version").output().is_ok();
    let path = dir.join(entry);
    match (path.is_file() && xmllint, required) {
        (true, _) => Some(path),
        (false, true) => panic!("{var_required}=1 but the schema or xmllint is missing (run `just schema`)"),
        (false, false) => {
            eprintln!("SKIPPED: {entry} or xmllint not available — run `just schema`");
            None
        }
    }
}

fn xmllint(schema: &Path, files: &[PathBuf]) -> Vec<String> {
    let mut failures = Vec::new();
    for chunk in files.chunks(200) {
        // No network access for the validator.
        let out = Command::new("xmllint")
            .args(["--nonet", "--noout", "--schema"])
            .arg(schema)
            .args(chunk)
            .output()
            .expect("xmllint runs");
        let stderr = String::from_utf8_lossy(&out.stderr);
        failures.extend(stderr.lines().filter(|l| !l.ends_with(" validates")).map(str::to_owned));
    }
    failures
}

#[test]
fn every_scenario_payload_is_valid_against_the_official_schemas() {
    // sandbox/SR-004
    let wbci = schema("WBCI_XSD_DIR", "target/wbci-xsd", "wsdl/wbci-envelope.xsd", "WBCI_XSD_REQUIRED");
    let spri = schema("SPRI_XSD_DIR", "target/spri-xsd", "wsdl/oss-envelope.xsd", "SPRI_XSD_REQUIRED");
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join("sim-corpus");
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let (mut w, mut s) = (Vec::new(), Vec::new());
    let other: telco_interop::wbci::Timestamp = "2027-12-23T16:00:00Z".parse().unwrap();
    for (run, start) in [sim::START, other].into_iter().enumerate() {
        for sz in sim::katalog() {
            let ablauf = sz.play(start);
            assert!(ablauf.abgelehnt.is_empty(), "{}: {:?}", sz.name, ablauf.abgelehnt);
            for (i, n) in ablauf.nachrichten.iter().enumerate() {
                let path = dir.join(format!("{run}-{}-{}", sz.name, n.file_name(i + 1)));
                std::fs::write(&path, n.to_xml().unwrap()).unwrap();
                match n.inhalt {
                    Inhalt::Wbci(_) => w.push(path),
                    Inhalt::Spri(_) => s.push(path),
                }
            }
        }
    }
    if let Some(schema) = wbci {
        let failures = xmllint(&schema, &w);
        assert!(failures.is_empty(), "{failures:#?}");
        eprintln!("{} WBCI scenario payloads valid against XSD V2.00", w.len());
    }
    if let Some(schema) = spri {
        let failures = xmllint(&schema, &s);
        assert!(failures.is_empty(), "{failures:#?}");
        eprintln!("{} S/PRI scenario payloads valid against XSD 4.00.03", s.len());
    }
}
