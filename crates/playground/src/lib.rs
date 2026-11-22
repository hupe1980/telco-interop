//! The site's playground: [`telco_interop::report`] compiled to WebAssembly, so a visitor can
//! read, check and explain a WBCI or S/PRI payload in the browser. Not published.
//!
//! [`run`] is the whole behaviour, a pure function over bytes, and [`scenario`] gives the sandbox
//! view; the `abi` module (compiled for `wasm32` only) hands bytes in and text out through two
//! module-owned buffers. The output is exactly the lines `telco-interop read`, `check` and
//! `code` print, joined by `\n`.
//!
//! # Why this crate allows `unsafe_code` at all
//!
//! A WebAssembly export needs an unmangled symbol name. Edition 2024 writes that attribute as
//! `#[unsafe(no_mangle)]`, and the `unsafe_code` lint flags it in every form (`export_name`
//! too, and the attribute wasm-bindgen generates). So this crate — and only this one, which is
//! never published — sets the lint to `deny` instead of the workspace's `forbid` and allows it
//! on each exported function. There is no `unsafe` block and no `unsafe fn`: the script writes
//! into a buffer the module handed out, between calls, while no Rust borrow of it is alive, and
//! every length the script passes is clamped to that buffer.

use telco_interop::{report, spri, wbci};

/// The largest input accepted: one byte more than either reader takes, so the reader itself
/// refuses an oversized payload with its limit error.
#[must_use]
pub fn input_limit() -> usize {
    wbci::xml::Limits::default().max_bytes.max(spri::xml::default_limits().max_bytes).saturating_add(1)
}

/// The first `len` bytes of `buffer`, or all of it when `len` is larger: a length from the
/// script never reaches past the buffer the module owns.
#[must_use]
pub fn prefix(buffer: &[u8], len: usize) -> &[u8] {
    buffer.get(..len).unwrap_or(buffer)
}

/// Which interfaces a code lookup covers (`telco-interop code`, `--wbci`, `--spri`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Scope {
    /// Both interfaces.
    Both,
    /// WBCI only.
    Wbci,
    /// S/PRI only.
    Spri,
}

impl Scope {
    /// `1` WBCI, `2` S/PRI, anything else both.
    #[must_use]
    pub const fn from_u32(n: u32) -> Self {
        match n {
            1 => Self::Wbci,
            2 => Self::Spri,
            _ => Self::Both,
        }
    }
}

/// A command of the playground.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Command {
    /// `telco-interop read`.
    Read,
    /// `telco-interop check`.
    Check,
    /// `telco-interop code`; the input is the code.
    Code(Scope),
}

/// Runs `command` on `input` (at most [`input_limit`] bytes are looked at; a code is decoded
/// lossily): whether it passed (the command line's exit code 0) and its lines joined by `\n`.
#[must_use]
pub fn run(command: Command, input: &[u8]) -> (bool, String) {
    let input = prefix(input, input_limit());
    let out = match command {
        Command::Read => report::read(input),
        Command::Check => report::check(input),
        Command::Code(scope) => {
            report::code(&String::from_utf8_lossy(input), scope != Scope::Spri, scope != Scope::Wbci)
        }
    };
    (out.ok, out.to_string())
}

/// A synthetic example payload, from the repository's test fixtures.
#[derive(Debug, Clone, Copy)]
pub struct Example {
    /// What the dropdown shows, e.g. `WBCI · ABBM (refusal)`.
    pub label: &'static str,
    /// The payload.
    pub payload: &'static str,
}

macro_rules! example {
    ($label:literal, $interface:literal, $file:literal) => {
        Example { label: $label, payload: include_str!(concat!("../../", $interface, "/tests/fixtures/", $file)) }
    };
}

/// The examples, compiled in so that choosing one fetches nothing.
pub const EXAMPLES: &[Example] = &[
    example!("WBCI · VA-KUE-MRN (pre-agreement)", "wbci", "va-kue-mrn.xml"),
    example!("WBCI · RUEM-VA (agreed)", "wbci", "ruem-va.xml"),
    example!("WBCI · ABBM (refusal)", "wbci", "abbm.xml"),
    example!("WBCI · AKM-TR (line taken over)", "wbci", "akm-tr.xml"),
    example!("WBCI · STR-AUF by the EKPabg (a warning)", "wbci", "str-auf-ekpabg.xml"),
    example!("WBCI · TEQ for a request", "wbci", "teq-anfrage.xml"),
    example!("S/PRI · PV order (provider switch)", "spri", "auftrag-pv.xml"),
    example!("S/PRI · NEU order", "spri", "auftrag-neu.xml"),
    example!("S/PRI · QEB", "spri", "meldung-qeb.xml"),
    example!("S/PRI · RUEM-PV", "spri", "meldung-ruem-pv.xml"),
    example!("S/PRI · ABBM", "spri", "meldung-abbm.xml"),
    example!("S/PRI · TEQ for an order", "spri", "teq-auftrag.xml"),
];

/// Scenario `index` of the sandbox catalogue: its title line (`part` 0) or its messages, one
/// line each, played from [`telco_interop::sim::START`] (else), or `None` past the end.
#[must_use]
pub fn scenario(index: usize, part: u32) -> Option<String> {
    let s = telco_interop::sim::katalog().get(index)?;
    if part == 0 {
        return Some(format!("{} — {} · {}", s.name, s.beschreibung, s.quelle));
    }
    let ablauf = s.play(telco_interop::sim::START);
    let mut lines: Vec<String> = ablauf.nachrichten.iter().map(|n| report::sanitize(&n.to_string())).collect();
    lines.extend(ablauf.abgelehnt.iter().map(|r| report::sanitize(&format!("refused: {r}"))));
    Some(lines.join("\n"))
}

/// The exports the page script calls (see `site/static/playground/playground.js`).
///
/// Protocol: `pg_input(n)` sizes the input buffer to `min(n, pg_input_limit())` zero bytes and
/// returns its address; the script writes that many bytes there; `pg_read(n)`, `pg_check(n)` or
/// `pg_code(n, scope)` runs on the first `n` of them (clamped to the buffer) and returns `0`
/// (passed) or `1` (failed); the text is at `pg_output_ptr()`, `pg_output_len()` bytes long.
/// `pg_example(i, part)` puts example `i`'s label (`part` 0) or payload (else) into the output
/// buffer and returns `1`, or `0` past the end; `pg_scenario(i, part)` does the same for the
/// sandbox scenario's title or messages.
#[cfg(target_arch = "wasm32")]
pub mod abi {
    use std::sync::{Mutex, MutexGuard, PoisonError};

    use super::{Command, EXAMPLES, Scope, input_limit, prefix, run};

    static INPUT: Mutex<Vec<u8>> = Mutex::new(Vec::new());
    static OUTPUT: Mutex<Vec<u8>> = Mutex::new(Vec::new());

    /// The buffer; a poisoned lock (impossible with `panic = "abort"`) is taken over as it is.
    fn lock(m: &Mutex<Vec<u8>>) -> MutexGuard<'_, Vec<u8>> {
        m.lock().unwrap_or_else(PoisonError::into_inner)
    }

    fn command(c: Command, len: usize) -> u32 {
        let (ok, text) = {
            let input = lock(&INPUT);
            run(c, prefix(&input, len))
        };
        *lock(&OUTPUT) = text.into_bytes();
        u32::from(!ok)
    }

    /// The largest input accepted.
    // SAFETY: the unmangled name `pg_input_limit` is unique in the module; the body is safe Rust.
    #[allow(unsafe_code)]
    #[unsafe(no_mangle)]
    pub extern "C" fn pg_input_limit() -> usize {
        input_limit()
    }

    /// Sizes the input buffer to `min(len, limit)` zero bytes and returns its address.
    // SAFETY: the unmangled name `pg_input` is unique in the module; the body is safe Rust and
    // only hands out the address of a buffer it owns.
    #[allow(unsafe_code)]
    #[unsafe(no_mangle)]
    pub extern "C" fn pg_input(len: usize) -> *mut u8 {
        let mut input = lock(&INPUT);
        input.clear();
        input.resize(len.min(input_limit()), 0);
        input.as_mut_ptr()
    }

    /// `telco-interop read` on the first `len` input bytes.
    // SAFETY: the unmangled name `pg_read` is unique in the module; the body is safe Rust.
    #[allow(unsafe_code)]
    #[unsafe(no_mangle)]
    pub extern "C" fn pg_read(len: usize) -> u32 {
        command(Command::Read, len)
    }

    /// `telco-interop check` on the first `len` input bytes.
    // SAFETY: the unmangled name `pg_check` is unique in the module; the body is safe Rust.
    #[allow(unsafe_code)]
    #[unsafe(no_mangle)]
    pub extern "C" fn pg_check(len: usize) -> u32 {
        command(Command::Check, len)
    }

    /// `telco-interop code` on the first `len` input bytes; `scope` 1 WBCI, 2 S/PRI, else both.
    // SAFETY: the unmangled name `pg_code` is unique in the module; the body is safe Rust.
    #[allow(unsafe_code)]
    #[unsafe(no_mangle)]
    pub extern "C" fn pg_code(len: usize, scope: u32) -> u32 {
        command(Command::Code(Scope::from_u32(scope)), len)
    }

    /// Example `index`'s label (`part` 0) or payload into the output buffer: `1`, or `0` past
    /// the end.
    // SAFETY: the unmangled name `pg_example` is unique in the module; the body is safe Rust.
    #[allow(unsafe_code)]
    #[unsafe(no_mangle)]
    pub extern "C" fn pg_example(index: usize, part: u32) -> u32 {
        let Some(e) = EXAMPLES.get(index) else { return 0 };
        let text = if part == 0 { e.label } else { e.payload };
        *lock(&OUTPUT) = text.as_bytes().to_vec();
        1
    }

    /// Scenario `index`'s title (`part` 0) or timeline (else) into the output buffer: `1`, or
    /// `0` past the end.
    // SAFETY: the unmangled name `pg_scenario` is unique in the module; the body is safe Rust.
    #[allow(unsafe_code)]
    #[unsafe(no_mangle)]
    pub extern "C" fn pg_scenario(index: usize, part: u32) -> u32 {
        let Some(text) = super::scenario(index, part) else { return 0 };
        *lock(&OUTPUT) = text.into_bytes();
        1
    }

    /// The address of the output text.
    // SAFETY: the unmangled name `pg_output_ptr` is unique in the module; the body is safe Rust.
    #[allow(unsafe_code)]
    #[unsafe(no_mangle)]
    pub extern "C" fn pg_output_ptr() -> *const u8 {
        lock(&OUTPUT).as_ptr()
    }

    /// The length of the output text in bytes.
    // SAFETY: the unmangled name `pg_output_len` is unique in the module; the body is safe Rust.
    #[allow(unsafe_code)]
    #[unsafe(no_mangle)]
    pub extern "C" fn pg_output_len() -> usize {
        lock(&OUTPUT).len()
    }
}
