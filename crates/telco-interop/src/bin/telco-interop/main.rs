//! `telco-interop`: read and check WBCI and S/PRI files, explain a code, replay the files of one
//! provider switch as a timeline with findings and deadlines, and write a sandbox scenario's
//! files.
//!
//! The binary is the I/O boundary of the sans-I/O crates: it reads the files it is given and
//! prints to standard output; only `simulate` writes files, into the folder it is given. It
//! opens no connection and reads no clock (the replay's "now" is `--now` or the latest control
//! timestamp; a scenario starts at `--start` or `telco_interop::sim::START`). Every printed line
//! passes [`out`], which replaces control characters taken from the input. What `read`, `check`
//! and `code` print comes from `telco_interop::report` (shared with the site's playground):
//! kinds, ids, dates and codes, never names, addresses, phone numbers or free text.

use std::fmt::Display;
use std::io::Write as _;
use std::path::PathBuf;
use std::process::ExitCode;

use clap::{Parser, Subcommand, ValueEnum};
use telco_interop::{Partei, report, wbci};

mod input;
mod simulate;
mod timeline;

/// Read, check and explain WBCI and S/PRI files; replay a provider switch; simulate one.
#[derive(Parser)]
#[command(name = "telco-interop", version, about = "Read, check and explain WBCI and S/PRI files; replay a provider switch; simulate one.", long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// What each file is — interface, kind, business case, ids, dates, codes — or why it
    /// cannot be read and the TEQ code to answer with.
    Read {
        /// WBCI or S/PRI payloads, bare or in a SOAP 1.1 envelope.
        #[arg(required = true)]
        files: Vec<PathBuf>,
    },
    /// The validation issues of each WBCI or S/PRI file (request, order, message or receipt),
    /// or why it cannot be read; fails on an error.
    Check {
        /// WBCI or S/PRI payloads, bare or in a SOAP 1.1 envelope.
        #[arg(required = true)]
        files: Vec<PathBuf>,
    },
    /// The meaning of a Meldungscode in WBCI and S/PRI.
    Code {
        /// The code, e.g. 8103 or 0021.
        code: String,
        /// WBCI only.
        #[arg(long, conflicts_with = "spri")]
        wbci: bool,
        /// S/PRI only.
        #[arg(long)]
        spri: bool,
    },
    /// Replay the files of one switch from one party's view: timeline, findings, deadlines.
    Timeline {
        /// Files, or folders whose regular files are read (no symbolic links, no sub-folders,
        /// at most 10 000 entries).
        #[arg(required = true)]
        inputs: Vec<PathBuf>,
        /// The party whose view is replayed: a file it sent is Sent, any other Received.
        #[arg(long = "as", value_enum)]
        party: Party,
        /// The moment deadlines are judged at (RFC 3339); default: the latest control
        /// timestamp of the files.
        #[arg(long, value_parser = parse_now)]
        now: Option<wbci::Timestamp>,
    },
    /// Write the files of a sandbox scenario (synthetic data, both sides, names in time order),
    /// or list the scenarios.
    Simulate {
        /// The scenario, e.g. spri-pv (see --list).
        #[arg(required_unless_present = "list")]
        scenario: Option<String>,
        /// The folder to write into; created if missing.
        #[arg(required_unless_present = "list")]
        out_dir: Option<PathBuf>,
        /// List the scenarios with what they show and their source.
        #[arg(long, conflicts_with_all = ["scenario", "out_dir", "force", "start"])]
        list: bool,
        /// Replace files of the same name (never writing through a symbolic link).
        #[arg(long)]
        force: bool,
        /// When the scenario starts: an RFC 3339 timestamp, or a date (09:00 German time that
        /// day); default: Monday 01.03.2027 09:00 German time.
        #[arg(long, value_parser = parse_start)]
        start: Option<wbci::Timestamp>,
    },
}

#[derive(Clone, Copy, ValueEnum)]
enum Party {
    /// The gaining provider: WBCI EKPauf, S/PRI Auftraggeber.
    Ekpauf,
    /// The losing provider: WBCI EKPabg, S/PRI abgebender Provider.
    Ekpabg,
    /// The network owner: S/PRI Leistungserbringer.
    Netzbetreiber,
}

impl From<Party> for Partei {
    fn from(p: Party) -> Self {
        match p {
            Party::Ekpauf => Self::EkpAuf,
            Party::Ekpabg => Self::EkpAbg,
            Party::Netzbetreiber => Self::Netzbetreiber,
        }
    }
}

fn parse_now(s: &str) -> Result<wbci::Timestamp, String> {
    s.parse().map_err(|e| format!("not an RFC 3339 timestamp: {e}"))
}

/// A timestamp, or a date meaning 09:00 German time that day (the default start's time of day).
fn parse_start(s: &str) -> Result<wbci::Timestamp, String> {
    if let Ok(day) = s.parse::<wbci::Date>() {
        let midnight = carrier_core::count::start_of(day).as_second();
        return wbci::Timestamp::from_second(midnight.saturating_add(9 * 3600))
            .map_err(|_| format!("{s} is out of range"));
    }
    parse_now(s).map_err(|e| format!("{e} (or a date, e.g. 2027-03-01)"))
}

/// Prints one line. Every character from the input passes here: control characters (C0, DEL,
/// C1) become U+FFFD, so no file can move the cursor, clear the screen or hide output.
fn out(line: impl Display) {
    let text = report::sanitize(&line.to_string());
    // A closed pipe (`| head`) ends the output; there is nothing left to report it to.
    let _ = writeln!(std::io::stdout().lock(), "{text}");
}

fn main() -> ExitCode {
    let cli = Cli::parse();
    let ok = match cli.command {
        Command::Read { files } => read(&files),
        Command::Check { files } => check(&files),
        Command::Code { code, wbci, spri } => code_cmd(&code, !spri, !wbci),
        Command::Timeline { inputs, party, now } => timeline_cmd(&inputs, party.into(), now),
        Command::Simulate { scenario, out_dir, list, force, start } => {
            simulate_cmd(scenario.as_deref(), out_dir.as_deref(), list, force, start)
        }
    };
    if ok { ExitCode::SUCCESS } else { ExitCode::FAILURE }
}

// -- read, check ----------------------------------------------------------------------------

/// Prints each file's path and the lines of `render` on its bytes, indented; an unopenable file
/// is `unreadable` with the reason.
fn per_file(files: &[PathBuf], render: fn(&[u8]) -> report::Output) -> bool {
    let mut ok = true;
    for path in files {
        out(path.display());
        match input::read_bytes(path) {
            Ok(bytes) => {
                let r = render(&bytes);
                ok &= r.ok;
                for line in &r.lines {
                    out(format_args!("  {line}"));
                }
            }
            Err(reason) => {
                ok = false;
                out(format_args!("  unreadable: {reason}"));
            }
        }
    }
    ok
}

fn read(files: &[PathBuf]) -> bool {
    per_file(files, report::read)
}

fn check(files: &[PathBuf]) -> bool {
    per_file(files, report::check)
}

// -- code -----------------------------------------------------------------------------------

fn code_cmd(code: &str, in_wbci: bool, in_spri: bool) -> bool {
    let r = report::code(code, in_wbci, in_spri);
    for line in &r.lines {
        out(line);
    }
    r.ok
}

// -- simulate -------------------------------------------------------------------------------

fn simulate_cmd(
    scenario: Option<&str>,
    out_dir: Option<&std::path::Path>,
    list: bool,
    force: bool,
    start: Option<wbci::Timestamp>,
) -> bool {
    if list {
        for s in telco_interop::sim::katalog() {
            out(format_args!("{} — {} · {}", s.name, s.beschreibung, s.quelle));
        }
        return true;
    }
    let (Some(name), Some(dir)) = (scenario, out_dir) else {
        out("error: give a scenario and a folder, or --list");
        return false;
    };
    let Some(found) = telco_interop::sim::szenario(name) else {
        out(format_args!("error: no scenario {name} (see simulate --list)"));
        return false;
    };
    match simulate::write(found, dir, start.unwrap_or(telco_interop::sim::START), force) {
        Ok(paths) => {
            for p in &paths {
                out(format_args!("wrote {}", p.display()));
            }
            out(format_args!(
                "{} files of scenario {}; replay them with: telco-interop timeline {} --as ekpauf",
                paths.len(),
                found.name,
                dir.display()
            ));
            true
        }
        Err(e) => {
            out(format_args!("error: {e}"));
            false
        }
    }
}

// -- timeline -------------------------------------------------------------------------------

fn timeline_cmd(inputs: &[PathBuf], party: Partei, now: Option<wbci::Timestamp>) -> bool {
    let given = now.is_some();
    let report = match timeline::replay(inputs, party, now) {
        Ok(r) => r,
        Err(e) => {
            out(format_args!("error: {e}"));
            return false;
        }
    };
    match &report.id {
        Some(id) => out(format_args!("Switch {id}, replayed as {party}")),
        None => out(format_args!("No switch: no file carries a Vorabstimmungs-ID (replayed as {party})")),
    }
    if let Some(now) = report.now {
        let source = if given { "--now" } else { "the latest control timestamp" };
        out(format_args!("Now: {now} ({source})"));
    }
    out("Timeline:");
    if report.timeline.is_empty() {
        out("  (empty)");
    }
    for e in &report.timeline {
        out(format_args!("  {e}"));
    }
    if report.befunde.is_empty() {
        out(format_args!("Findings: none{}", report.remark.map_or_else(String::new, |r| format!(" — {r}"))));
    } else {
        out("Findings:");
        for b in &report.befunde {
            out(format_args!("  {b}"));
        }
    }
    if report.deadlines.is_empty() {
        out("Deadlines: none open");
    } else {
        out("Deadlines:");
        for (overdue, f) in &report.deadlines {
            out(format_args!("  {} {f}", if *overdue { "OVERDUE" } else { "open   " }));
        }
    }
    for (title, notes) in
        [("Skipped", &report.skipped), ("Refused", &report.refused), ("Unreadable", &report.unreadable)]
    {
        if !notes.is_empty() {
            out(format_args!("{title}:"));
            for n in notes {
                out(format_args!("  {} — {}", n.path.display(), n.reason));
            }
        }
    }
    !report.failed()
}
