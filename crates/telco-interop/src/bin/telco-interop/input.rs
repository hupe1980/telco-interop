//! Reading files: bounded reads and the bounded folder listing; the interface detection is
//! `telco_interop::report::Payload::parse`.

use std::fs::{self, File};
use std::io::Read as _;
use std::path::{Path, PathBuf};

use telco_interop::report::{Payload, Unreadable};
use telco_interop::{spri, wbci};

/// The most entries a replay lists, over all folders and files given.
pub(crate) const MAX_FILES: usize = 10_000;

/// Why a file could not be read.
pub(crate) enum Failure {
    /// Not opened: missing, not a regular file, no permission.
    Io(String),
    /// Opened, refused by both readers.
    Read(Unreadable),
}

impl Failure {
    /// One line: the reason and, for a refused payload, the receipt code to answer with.
    pub(crate) fn describe(&self) -> String {
        match self {
            Self::Io(reason) => reason.clone(),
            Self::Read(u) => u.to_string(),
        }
    }
}

/// The largest input either reader accepts; one byte more is read so the reader can refuse it.
fn max_bytes() -> usize {
    wbci::xml::Limits::default().max_bytes.max(spri::xml::default_limits().max_bytes)
}

/// Reads the bytes of one regular file (a named symbolic link is followed), at most one byte
/// more than either reader accepts.
pub(crate) fn read_bytes(path: &Path) -> Result<Vec<u8>, String> {
    let meta = fs::metadata(path).map_err(|e| format!("cannot open: {e}"))?;
    if !meta.is_file() {
        return Err("not a regular file".to_owned());
    }
    let file = File::open(path).map_err(|e| format!("cannot open: {e}"))?;
    let mut bytes = Vec::new();
    let limit = u64::try_from(max_bytes()).unwrap_or(u64::MAX).saturating_add(1);
    file.take(limit).read_to_end(&mut bytes).map_err(|e| format!("cannot read: {e}"))?;
    Ok(bytes)
}

/// Reads one regular file and detects its interface.
pub(crate) fn read_file(path: &Path) -> Result<Payload, Failure> {
    let bytes = read_bytes(path).map_err(Failure::Io)?;
    Payload::parse(&bytes).map_err(Failure::Read)
}

/// An entry of a folder that is not replayed.
pub(crate) struct Skipped {
    pub(crate) path: PathBuf,
    pub(crate) reason: &'static str,
}

/// The files to replay: each named file, and the regular files directly inside each named
/// folder (sorted by name). Symbolic links inside a folder are not followed, sub-folders not
/// searched; both are reported in `skipped`.
///
/// # Errors
///
/// More than [`MAX_FILES`] entries, or a folder that cannot be listed.
pub(crate) fn list(inputs: &[PathBuf], skipped: &mut Vec<Skipped>) -> Result<Vec<PathBuf>, String> {
    let mut files = Vec::new();
    let mut seen = 0usize;
    let mut count = |n: usize| {
        seen = seen.saturating_add(n);
        if seen > MAX_FILES {
            Err(format!("more than {MAX_FILES} files: replay refused; split the folder"))
        } else {
            Ok(())
        }
    };
    for input in inputs {
        let is_dir = fs::metadata(input).is_ok_and(|m| m.is_dir());
        if !is_dir {
            count(1)?;
            files.push(input.clone());
            continue;
        }
        let entries = fs::read_dir(input).map_err(|e| format!("{}: cannot list: {e}", input.display()))?;
        let mut here = Vec::new();
        for entry in entries {
            count(1)?;
            let entry = entry.map_err(|e| format!("{}: cannot list: {e}", input.display()))?;
            let path = entry.path();
            match entry.file_type() {
                Ok(t) if t.is_file() => here.push(path),
                Ok(t) if t.is_symlink() => skipped.push(Skipped { path, reason: "symbolic link, not followed" }),
                Ok(t) if t.is_dir() => skipped.push(Skipped { path, reason: "sub-folder, not searched" }),
                Ok(_) => skipped.push(Skipped { path, reason: "not a regular file" }),
                Err(_) => skipped.push(Skipped { path, reason: "file type unknown" }),
            }
        }
        here.sort();
        files.extend(here);
    }
    skipped.sort_by(|a, b| a.path.cmp(&b.path));
    Ok(files)
}
