//! Reading files: the bounded folder listing and the interface detection.

use std::fs::{self, File};
use std::io::Read as _;
use std::path::{Path, PathBuf};

use telco_interop::{spri, wbci};

/// The most entries a replay lists, over all folders and files given.
pub(crate) const MAX_FILES: usize = 10_000;

/// The envelope namespaces: the root of every payload is in one of them.
const NS_WBCI: &[u8] = b"http://www.vatm.de/wbci/v1/envelope";
const NS_SPRI: &[u8] = b"http://spri.telekom.de/oss/v4/envelope";

/// A payload of either interface.
pub(crate) enum Parsed {
    Wbci(Box<wbci::Payload>),
    Spri(Box<spri::Payload>),
}

impl Parsed {
    /// The control timestamp.
    pub(crate) fn at(&self) -> wbci::Timestamp {
        match self {
            Self::Wbci(p) => p.control().zeitstempel.timestamp,
            Self::Spri(p) => spri_control(p).zeitstempel.timestamp,
        }
    }
}

pub(crate) const fn spri_control(p: &spri::Payload) -> &spri::Control {
    match p {
        spri::Payload::AuftragRequest { control, .. }
        | spri::Payload::AuftragResponse { control, .. }
        | spri::Payload::MeldungRequest { control, .. }
        | spri::Payload::MeldungResponse { control, .. } => control,
    }
}

/// Why a file could not be read.
pub(crate) enum Failure {
    /// Not opened: missing, not a regular file, no permission.
    Io(String),
    /// Opened, refused by the reader of `interface` (`None`: neither interface's namespace).
    Read { interface: Option<&'static str>, reason: String, teq: String },
}

impl Failure {
    /// One line: the reason and, for a refused payload, the receipt code to answer with.
    pub(crate) fn describe(&self) -> String {
        match self {
            Self::Io(reason) => reason.clone(),
            Self::Read { interface: Some(i), reason, teq } => format!("{i}: {reason} — answer with TEQ {teq}"),
            Self::Read { interface: None, reason, teq } => {
                format!("neither a WBCI nor an S/PRI payload: {reason} — answer with TEQ {teq}")
            }
        }
    }
}

/// The largest input either reader accepts; one byte more is read so the reader can refuse it.
fn max_bytes() -> usize {
    wbci::xml::Limits::default().max_bytes.max(spri::xml::default_limits().max_bytes)
}

/// Reads one regular file (a named symbolic link is followed) and detects its interface.
pub(crate) fn read_file(path: &Path) -> Result<Parsed, Failure> {
    let meta = fs::metadata(path).map_err(|e| Failure::Io(format!("cannot open: {e}")))?;
    if !meta.is_file() {
        return Err(Failure::Io("not a regular file".to_owned()));
    }
    let file = File::open(path).map_err(|e| Failure::Io(format!("cannot open: {e}")))?;
    let mut bytes = Vec::new();
    let limit = u64::try_from(max_bytes()).unwrap_or(u64::MAX).saturating_add(1);
    file.take(limit).read_to_end(&mut bytes).map_err(|e| Failure::Io(format!("cannot read: {e}")))?;
    parse(&bytes)
}

/// Tries both readers; a failure is attributed to the interface whose envelope namespace the
/// input names.
pub(crate) fn parse(bytes: &[u8]) -> Result<Parsed, Failure> {
    let wbci_err = match wbci::Payload::from_xml(bytes) {
        Ok(p) => return Ok(Parsed::Wbci(Box::new(p))),
        Err(e) => e,
    };
    let spri_err = match spri::Payload::from_xml(bytes) {
        Ok(p) => return Ok(Parsed::Spri(Box::new(p))),
        Err(e) => e,
    };
    let names = |ns: &[u8]| bytes.windows(ns.len()).any(|w| w == ns);
    Err(if names(NS_WBCI) {
        Failure::Read { interface: Some("WBCI"), reason: wbci_err.to_string(), teq: wbci_err.teq_code().to_string() }
    } else if names(NS_SPRI) {
        Failure::Read { interface: Some("S/PRI"), reason: spri_err.to_string(), teq: spri_err.teq_code().to_string() }
    } else {
        Failure::Read { interface: None, reason: wbci_err.to_string(), teq: wbci_err.teq_code().to_string() }
    })
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
