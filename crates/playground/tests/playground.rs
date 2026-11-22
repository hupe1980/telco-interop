//! The playground's entry functions, compiled natively: hostile input, personal data, the
//! command line's lines, the examples, and static checks of the page script and template.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::indexing_slicing)]

use std::fs;
use std::path::{Path, PathBuf};

use playground::{Command, EXAMPLES, Scope, input_limit, prefix, run};

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn all_fixtures() -> Vec<PathBuf> {
    let mut v = Vec::new();
    for interface in ["wbci", "spri"] {
        let mut here: Vec<PathBuf> = fs::read_dir(root().join("crates").join(interface).join("tests/fixtures"))
            .unwrap()
            .map(|e| e.unwrap().path())
            .collect();
        here.sort();
        v.extend(here);
    }
    v
}

fn fixture(interface: &str, name: &str) -> String {
    fs::read_to_string(root().join("crates").join(interface).join("tests/fixtures").join(name)).unwrap()
}

#[track_caller]
fn unreadable(input: &[u8]) {
    for c in [Command::Read, Command::Check] {
        let (ok, text) = run(c, input);
        assert!(!ok, "{c:?}: {text}");
        assert_eq!(text.lines().count(), 1, "{c:?}: {text}");
        assert!(text.starts_with("unreadable: ") && text.contains("answer with TEQ "), "{c:?}: {text}");
    }
}

// -- hostile input --------------------------------------------------------------------------

#[test]
fn hostile_input_yields_one_error_line() {
    // playground/SR-004
    let abbm = fixture("wbci", "abbm.xml");
    unreadable(format!("<!DOCTYPE x [<!ENTITY a \"b\">]>\n{abbm}").as_bytes());
    unreadable(b"");
    unreadable(b"not xml at all");
    unreadable(&[0xff, 0xfe, 0x00, 0x80, b'<']);
    unreadable(b"\0\x1b[2J<a\x07/>");
    unreadable(&"<a>".repeat(10_000).into_bytes());
    // Above the readers' limit: refused by the reader's own size check.
    let mut huge = abbm.clone().into_bytes();
    huge.resize(input_limit() + 4096, b' ');
    unreadable(&huge);
    let (ok, text) = run(Command::Read, &huge);
    assert!(!ok && text.contains("WBCI"), "{text}");
}

#[test]
fn a_huge_code_answers() {
    // playground/SR-004
    let code = "9".repeat(1 << 20);
    let (ok, text) = run(Command::Code(Scope::Both), code.as_bytes());
    assert!(!ok);
    assert_eq!(text.lines().count(), 2);
    let (ok, _) = run(Command::Code(Scope::Wbci), &[0xff, 0x00]);
    assert!(!ok);
}

// -- value-free output ----------------------------------------------------------------------

#[test]
fn no_personal_data_or_control_characters_in_any_output() {
    // playground/SR-003
    let mut outputs = String::new();
    for file in all_fixtures() {
        let bytes = fs::read(&file).unwrap();
        for c in [Command::Read, Command::Check] {
            let (_, text) = run(c, &bytes);
            assert!(!text.chars().any(|ch| ch.is_control() && ch != '\n'), "{}: {text:?}", file.display());
            outputs.push_str(&text);
            outputs.push('\n');
        }
    }
    assert!(outputs.contains("interface: S/PRI") && outputs.contains("interface: WBCI"), "{outputs}");
    // The personal values of the fixtures, as listed in the command line's tests.
    for secret in [
        "Mustermann",
        "Erika",
        "Max ",
        "Beispiel",
        "Vormann",
        "Hauptstraße",
        "10115",
        "10117",
        "Berlin",
        "Mitte",
        "1234567",
        "1234568",
        "7654321",
        "9876543",
        "030 1234",
        "example.de",
        "example.org",
        "Neuer Anbieter",
        "Alter Anbieter",
        "RN unbekannt",
        "Umzug",
        "Widerruf",
        "Netzumbau",
        "Tiefbau",
        "Spleiß",
        "Kunde ist ab",
        "ONT wurde",
        "Bitte vorher",
        "Kein Sync",
    ] {
        assert!(!outputs.contains(secret), "{secret} in:\n{outputs}");
    }
    // Control characters from the input arrive as U+FFFD.
    let evil = fixture("wbci", "abbm.xml")
        .replace("<Meldungscode>8103</Meldungscode>", "<Meldungscode>\u{1b}[2J</Meldungscode>");
    for c in [Command::Read, Command::Check] {
        let (_, text) = run(c, evil.as_bytes());
        assert!(!text.contains('\u{1b}') && text.contains("\u{FFFD}[2J"), "{text:?}");
    }
}

// -- the command line's lines ---------------------------------------------------------------

#[test]
fn read_of_an_abbm() {
    let (ok, text) = run(Command::Read, fixture("wbci", "abbm.xml").as_bytes());
    assert!(ok);
    for line in
        ["interface: WBCI", "kind: ABBM", "business case: VA-KUE-MRN", "code: 8103 Rufnummer nicht geschaltet (RNG)"]
    {
        assert!(text.lines().any(|l| l == line), "{line} in {text}");
    }
}

#[test]
fn check_reports_warnings_and_errors() {
    let (ok, text) = run(Command::Check, fixture("spri", "auftrag-pv.xml").as_bytes());
    assert!(ok);
    assert_eq!(text, "S/PRI: no issues");
    let (ok, text) = run(Command::Check, fixture("wbci", "str-auf-ekpabg.xml").as_bytes());
    assert!(ok, "a warning passes: {text}");
    assert!(text.contains("warning [str-auf.ekpabg-grund]"), "{text}");
    let wrong = fixture("wbci", "ruem-va.xml")
        .replace("<Meldungscode>8001</Meldungscode>", "<Meldungscode>8103</Meldungscode>");
    let (ok, text) = run(Command::Check, wrong.as_bytes());
    assert!(!ok);
    assert!(text.contains("error ["), "{text}");
}

#[test]
fn code_lookup() {
    let (ok, text) = run(Command::Code(Scope::Both), b"8103");
    assert!(ok);
    for line in ["WBCI 8103 Rufnummer nicht geschaltet (RNG)", "  short name: RNG", "S/PRI: 8103 is not an S/PRI code"]
    {
        assert!(text.lines().any(|l| l == line), "{line} in {text}");
    }
    let (ok, text) = run(Command::Code(Scope::Spri), b"0021");
    assert!(ok && !text.contains("WBCI"), "{text}");
    assert!(text.contains("message types: RUEM-PV"), "{text}");
    let (ok, text) = run(Command::Code(Scope::Both), b"OK");
    assert!(ok && text.contains("WBCI OK OK.") && text.contains("S/PRI OK OK."), "{text}");
    let (ok, text) = run(Command::Code(Scope::Wbci), b"0021");
    assert!(!ok);
    assert_eq!(text, "WBCI: 0021 is not a WBCI code");
    assert_eq!(Scope::from_u32(7), Scope::Both);
}

// -- examples -------------------------------------------------------------------------------

#[test]
fn every_example_reads() {
    assert!(EXAMPLES.len() >= 10);
    let mut interfaces = String::new();
    for e in EXAMPLES {
        let (ok, text) = run(Command::Read, e.payload.as_bytes());
        assert!(ok, "{}: {text}", e.label);
        interfaces.push_str(text.lines().next().unwrap());
    }
    assert!(interfaces.contains("WBCI") && interfaces.contains("S/PRI"));
}

// -- the page -------------------------------------------------------------------------------

#[test]
fn the_page_script_stays_local_and_renders_text_only() {
    // playground/SR-001 playground/SR-002
    let js = fs::read_to_string(root().join("site/static/playground/playground.js")).unwrap();
    assert!(js.contains("textContent"));
    assert_eq!(js.matches("fetch(").count(), 1, "one fetch: the module itself");
    assert!(js.contains("new URL('playground.wasm', import.meta.url)"));
    for forbidden in [
        "innerHTML",
        "outerHTML",
        "insertAdjacentHTML",
        "document.write",
        "eval(",
        "Function(",
        "cookie",
        "localStorage",
        "sessionStorage",
        "indexedDB",
        "XMLHttpRequest",
        "sendBeacon",
        "WebSocket",
        "http:",
        "https:",
        "import(",
    ] {
        assert!(!js.contains(forbidden), "{forbidden} in playground.js");
    }
    let html = fs::read_to_string(root().join("site/templates/playground.html")).unwrap();
    for directive in [
        "default-src 'none'",
        "script-src 'self' 'wasm-unsafe-eval'",
        "connect-src 'self'",
        "form-action 'none'",
        "base-uri 'none'",
    ] {
        assert!(html.contains(directive), "CSP lacks {directive}");
    }
    assert!(html.contains("http-equiv=\"Content-Security-Policy\""));
    assert!(html.contains("<script type=\"module\" src="), "the script is an external module");
    assert_eq!(html.matches("<script").count(), 1, "no inline script");
    assert!(!html.contains("<form"), "no form");
    assert!(html.contains("aria-live=\"polite\"") && html.contains("<noscript>"));
}

#[test]
fn unsafe_code_only_as_the_export_attribute() {
    // playground/SR-006
    let this_test = Path::new(file!()).file_name().unwrap().to_owned();
    let mut stack = vec![root().join("crates")];
    while let Some(dir) = stack.pop() {
        for entry in fs::read_dir(&dir).unwrap() {
            let path = entry.unwrap().path();
            if path.is_dir() {
                stack.push(path);
                continue;
            }
            if path.extension().is_none_or(|e| e != "rs") || path.file_name() == Some(&this_test) {
                continue;
            }
            let text = fs::read_to_string(&path).unwrap();
            let lines: Vec<&str> = text.lines().collect();
            let is_abi = path.ends_with("crates/playground/src/lib.rs");
            for (i, line) in lines.iter().enumerate() {
                let code = line.split("//").next().unwrap();
                if !code.contains("unsafe") {
                    continue;
                }
                assert!(is_abi, "unsafe outside the playground's exports: {}:{}", path.display(), i + 1);
                let trimmed = code.trim();
                assert!(
                    trimmed == "#[unsafe(no_mangle)]" || trimmed == "#[allow(unsafe_code)]",
                    "only the export attribute: {}:{}: {line}",
                    path.display(),
                    i + 1
                );
                if trimmed == "#[unsafe(no_mangle)]" {
                    let before = &lines[i.saturating_sub(4)..i];
                    assert!(
                        before.iter().any(|l| l.trim().starts_with("// SAFETY:")),
                        "no SAFETY comment at line {}",
                        i + 1
                    );
                }
            }
        }
    }
    let manifest = fs::read_to_string(root().join("crates/playground/Cargo.toml")).unwrap();
    assert!(manifest.contains("unsafe_code                   = \"deny\"") && manifest.contains("publish      = false"));
}

#[test]
fn markup_in_the_input_comes_back_as_text() {
    // playground/SR-002
    // A Meldungscode is at most 10 characters: a short tag in the payload, a long one as a code.
    let xml = fixture("wbci", "abbm.xml")
        .replace("<Meldungscode>8103</Meldungscode>", "<Meldungscode>&lt;b&gt;x&lt;/b&gt;</Meldungscode>");
    for c in [Command::Read, Command::Check] {
        let (_, text) = run(c, xml.as_bytes());
        assert!(text.contains("<b>x</b>"), "{c:?}: {text}");
    }
    let markup = "<img src=x onerror=alert(1)>";
    let (ok, text) = run(Command::Code(Scope::Both), markup.as_bytes());
    assert!(!ok);
    assert_eq!(text, format!("WBCI: {markup} is not a WBCI code\nS/PRI: {markup} is not an S/PRI code"));
}

#[test]
fn a_length_past_the_buffer_reads_only_the_buffer() {
    // playground/SR-006
    let buffer = b"<a/>";
    assert_eq!(prefix(buffer, 2), b"<a");
    assert_eq!(prefix(buffer, 4), buffer);
    assert_eq!(prefix(buffer, usize::MAX), buffer);
    assert_eq!(prefix(&[], 1_000_000), b"");
    let huge = vec![b' '; input_limit() + 10];
    assert_eq!(prefix(&huge, input_limit()).len(), input_limit());
}

// -- the sandbox view -----------------------------------------------------------------------

#[test]
fn every_scenario_is_listed_and_shown_as_the_library_prints_it() {
    let catalogue = telco_interop::sim::katalog();
    for (i, s) in catalogue.iter().enumerate() {
        let title = playground::scenario(i, 0).unwrap();
        assert!(title.starts_with(s.name), "{title}");
        let lines = playground::scenario(i, 1).unwrap();
        let expected: Vec<String> =
            s.play(telco_interop::sim::START).nachrichten.iter().map(ToString::to_string).collect();
        assert_eq!(lines, expected.join("\n"));
        for secret in ["Musterfrau", "Erika", "Musterweg", "12345", "Musterstadt", "5550100"] {
            assert!(!lines.contains(secret), "{secret} in {}", s.name);
        }
    }
    assert!(playground::scenario(catalogue.len(), 0).is_none());
}
