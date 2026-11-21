//! `telco-interop`: read and check WBCI and S/PRI files, explain a code, and replay the files
//! of one provider switch as a timeline with findings and deadlines.
//!
//! The binary is the I/O boundary of the sans-I/O crates: it reads the files it is given and
//! prints to standard output; it writes no file, opens no connection and reads no clock (the
//! replay's "now" is `--now` or the latest control timestamp). Every printed line passes
//! [`out`], which replaces control characters taken from the input; summaries print kinds,
//! ids, dates and codes, never names, addresses, phone numbers or free text.

use std::fmt::Display;
use std::io::Write as _;
use std::path::PathBuf;
use std::process::ExitCode;

use clap::{Parser, Subcommand, ValueEnum};
use telco_interop::{Partei, spri, wbci};

mod input;
mod summary;
mod timeline;

use input::Parsed;

/// Read, check and explain WBCI and S/PRI files; replay a provider switch.
#[derive(Parser)]
#[command(name = "telco-interop", version, about = "Read, check and explain WBCI and S/PRI files; replay a provider switch.", long_about = None)]
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
    /// The validation issues of each WBCI request, message or receipt, and the read errors of
    /// files of both interfaces.
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

/// Prints one line. Every character from the input passes here: control characters (C0, DEL,
/// C1) become U+FFFD, so no file can move the cursor, clear the screen or hide output.
fn out(line: impl Display) {
    let text = sanitize(&line.to_string());
    // A closed pipe (`| head`) ends the output; there is nothing left to report it to.
    let _ = writeln!(std::io::stdout().lock(), "{text}");
}

fn sanitize(s: &str) -> String {
    s.chars().map(|c| if c.is_control() { '\u{FFFD}' } else { c }).collect()
}

fn main() -> ExitCode {
    let cli = Cli::parse();
    let ok = match cli.command {
        Command::Read { files } => read(&files),
        Command::Check { files } => check(&files),
        Command::Code { code, wbci, spri } => code_cmd(&code, !spri, !wbci),
        Command::Timeline { inputs, party, now } => timeline_cmd(&inputs, party.into(), now),
    };
    if ok { ExitCode::SUCCESS } else { ExitCode::FAILURE }
}

// -- read -----------------------------------------------------------------------------------

fn read(files: &[PathBuf]) -> bool {
    let mut ok = true;
    for path in files {
        out(path.display());
        match input::read_file(path) {
            Ok(p) => {
                for (label, value) in summary::summary(&p) {
                    out(format_args!("  {label}: {value}"));
                }
            }
            Err(f) => {
                ok = false;
                out(format_args!("  unreadable: {}", f.describe()));
            }
        }
    }
    ok
}

// -- check ----------------------------------------------------------------------------------

fn check(files: &[PathBuf]) -> bool {
    let mut ok = true;
    for path in files {
        out(path.display());
        let payload = match input::read_file(path) {
            Ok(p) => p,
            Err(f) => {
                ok = false;
                out(format_args!("  unreadable: {}", f.describe()));
                continue;
            }
        };
        let issues = match &payload {
            Parsed::Wbci(p) => match &**p {
                wbci::Payload::AnfrageRequest { anfrage, .. } => anfrage.issues(),
                wbci::Payload::MeldungRequest { meldung, .. } => meldung.issues(None),
                wbci::Payload::AnfrageResponse { quittung, .. } | wbci::Payload::MeldungResponse { quittung, .. } => {
                    quittung.issues()
                }
            },
            Parsed::Spri(_) => {
                out("  S/PRI: readable; the S/PRI crate has no validation beyond the reader");
                continue;
            }
        };
        if issues.is_empty() {
            out("  WBCI: no issues");
        }
        for issue in &issues {
            if issue.severity == wbci::Severity::Error {
                ok = false;
            }
            out(format_args!("  {issue}"));
        }
    }
    ok
}

// -- code -----------------------------------------------------------------------------------

fn code_cmd(code: &str, in_wbci: bool, in_spri: bool) -> bool {
    let mut known = false;
    if in_wbci {
        let c = wbci::Meldungscode::parse(code);
        if c.text().is_some() {
            known = true;
            out(format_args!("WBCI {c}"));
            if let Some(short) = c.short() {
                out(format_args!("  short name: {short}"));
            }
            let mut typen: Vec<wbci::Meldungstyp> = Vec::new();
            for t in c.meldungstypen() {
                if !typen.contains(&t) {
                    typen.push(t);
                }
            }
            for t in typen {
                let cases: Vec<&str> =
                    wbci::Geschaeftsfall::ALL.iter().filter(|g| c.applies(t, **g)).map(|g| g.as_str()).collect();
                out(format_args!("  {t} in {}", cases.join(", ")));
            }
        } else {
            out(format_args!("WBCI: {code} is not a WBCI code"));
        }
    }
    if in_spri {
        let c = spri::Meldungscode::parse(code);
        if c.text().is_some() {
            known = true;
            out(format_args!("S/PRI {c}"));
            let typen: Vec<&str> = c.meldungstypen().map(spri::Meldungstyp::as_str).collect();
            if !typen.is_empty() {
                out(format_args!("  message types: {}", typen.join(", ")));
            }
            let cases: Vec<&str> = c.geschaeftsfaelle().map(spri::GeschaeftsfallCode::as_str).collect();
            if !cases.is_empty() {
                out(format_args!("  business cases: {}", cases.join(", ")));
            }
        } else {
            out(format_args!("S/PRI: {code} is not an S/PRI code"));
        }
    }
    known
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
