//! Writing a sandbox scenario's payloads as files.

use std::fs::{self, OpenOptions};
use std::io::{ErrorKind, Write as _};
use std::path::{Path, PathBuf};

use telco_interop::sim::Szenario;
use telco_interop::wbci;

/// Writes the files of `szenario` played from `start` into `dir`, creating it if missing.
/// Returns the paths written, in time order.
///
/// Nothing is written if a target exists and `force` is not set. With `force` an existing
/// entry is removed first — a symbolic link itself, never what it points to — and every file is
/// created anew, so no link is ever written through.
///
/// # Errors
///
/// A target exists (without `force`), the folder cannot be created, or a file cannot be written.
pub(crate) fn write(
    szenario: &Szenario,
    dir: &Path,
    start: wbci::Timestamp,
    force: bool,
) -> Result<Vec<PathBuf>, String> {
    let ablauf = szenario.play(start);
    if let Some(why) = ablauf.abgelehnt.first() {
        return Err(format!("scenario {} did not play: {why}", szenario.name));
    }
    let files = ablauf.files().map_err(|e| format!("scenario {}: {e}", szenario.name))?;
    fs::create_dir_all(dir).map_err(|e| format!("{}: cannot create the folder: {e}", dir.display()))?;
    let targets: Vec<(PathBuf, String)> = files.into_iter().map(|(name, xml)| (dir.join(name), xml)).collect();
    // Check every target before writing any, so a refusal leaves the folder unchanged.
    for (path, _) in &targets {
        if fs::symlink_metadata(path).is_ok() && !force {
            return Err(format!("{} exists (use --force to replace it)", path.display()));
        }
    }
    let mut written = Vec::new();
    for (path, xml) in targets {
        if force {
            match fs::symlink_metadata(&path) {
                Ok(m) if m.is_dir() => return Err(format!("{} is a folder; not replaced", path.display())),
                Ok(_) => fs::remove_file(&path).map_err(|e| format!("{}: cannot replace: {e}", path.display()))?,
                Err(e) if e.kind() == ErrorKind::NotFound => {}
                Err(e) => return Err(format!("{}: {e}", path.display())),
            }
        }
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&path)
            .map_err(|e| format!("{}: cannot create: {e}", path.display()))?;
        file.write_all(xml.as_bytes()).map_err(|e| format!("{}: cannot write: {e}", path.display()))?;
        written.push(path);
    }
    Ok(written)
}
