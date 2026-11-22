//! The binary on the fixtures of both interfaces: each command's output and exit code, a
//! switch replayed from files written with the libraries, and the output's safety.
#![cfg(feature = "cli")]
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::indexing_slicing)]

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use telco_interop::{spri, wbci};

// -- harness --------------------------------------------------------------------------------

/// Runs the binary; returns standard output (and error, for usage errors) and the exit code.
fn run<S: AsRef<std::ffi::OsStr>>(args: &[S]) -> (String, i32) {
    let o = Command::new(env!("CARGO_BIN_EXE_telco-interop")).args(args).output().unwrap();
    let mut text = String::from_utf8(o.stdout).unwrap();
    text.push_str(&String::from_utf8(o.stderr).unwrap());
    (text, o.status.code().unwrap())
}

fn wbci_fixture(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../wbci/tests/fixtures").join(name)
}

fn spri_fixture(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../spri/tests/fixtures").join(name)
}

fn fixtures(dir: &Path) -> Vec<PathBuf> {
    let mut v: Vec<PathBuf> = fs::read_dir(dir).unwrap().map(|e| e.unwrap().path()).collect();
    v.sort();
    v
}

fn all_fixtures() -> Vec<PathBuf> {
    let mut v = fixtures(&wbci_fixture(""));
    v.extend(fixtures(&spri_fixture("")));
    v
}

/// A fresh, empty folder under the target directory.
fn temp(name: &str) -> PathBuf {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join("cli").join(name);
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    dir
}

#[track_caller]
fn contains_all(out: &str, parts: &[&str]) {
    for p in parts {
        assert!(out.contains(p), "missing {p:?} in:\n{out}");
    }
}

// -- read -----------------------------------------------------------------------------------

#[test]
fn read_names_kind_case_id_and_codes_of_a_wbci_message() {
    let (out, code) = run(&[wbci_fixture("abbm.xml")]);
    assert_eq!(code, 2, "a subcommand is required:\n{out}");
    let (out, code) = run(&[Path::new("read"), &wbci_fixture("abbm.xml")]);
    assert_eq!(code, 0, "{out}");
    contains_all(
        &out,
        &[
            "interface: WBCI",
            "kind: ABBM",
            "business case: VA-KUE-MRN",
            "Vorabstimmungs-ID: DEU.EKP1.V000000013",
            "sender: DEU.EKP2 (EKPabg)",
            "code: 8103 Rufnummer nicht geschaltet (RNG)",
        ],
    );
}

#[test]
fn read_names_an_spri_order_and_message() {
    let (out, code) = run(&[Path::new("read"), &spri_fixture("auftrag-pv.xml"), &spri_fixture("meldung-ruem-pv.xml")]);
    assert_eq!(code, 0, "{out}");
    contains_all(
        &out,
        &[
            "interface: S/PRI",
            "kind: PV",
            "externe Auftragsnummer: A-2026-0001",
            "Vorabstimmungs-ID: DEU.EKP1.V000000013",
            "Wunschtermin: 2027-01-15",
            "kind: RUEM-PV",
            "code: 0021 Antwort des abgebenden Providers.",
        ],
    );
}

#[test]
fn read_detects_the_interface_inside_soap() {
    let dir = temp("soap");
    let p = spri::Payload::from_xml(&fs::read(spri_fixture("meldung-qeb.xml")).unwrap()).unwrap();
    let file = dir.join("qeb-soap.xml");
    fs::write(&file, p.to_soap().unwrap()).unwrap();
    let w = wbci::Payload::from_xml(&fs::read(wbci_fixture("ruem-va.xml")).unwrap()).unwrap();
    let wfile = dir.join("ruem-va-soap.xml");
    fs::write(&wfile, w.to_soap().unwrap()).unwrap();
    let (out, code) = run(&[Path::new("read"), &file, &wfile]);
    assert_eq!(code, 0, "{out}");
    contains_all(&out, &["interface: S/PRI", "kind: QEB", "interface: WBCI", "kind: RUEM-VA", "code: 8001"]);
}

#[test]
fn a_doctype_is_refused_with_teq_0995() {
    let dir = temp("doctype");
    let file = dir.join("doctype.xml");
    let xml = fs::read_to_string(wbci_fixture("abbm.xml")).unwrap();
    fs::write(&file, format!("<!DOCTYPE x [<!ENTITY a \"b\">]>\n{xml}")).unwrap();
    let (out, code) = run(&[Path::new("read"), &wbci_fixture("abbm.xml"), &file]);
    assert_eq!(code, 1, "{out}");
    contains_all(&out, &["doctype.xml", "WBCI: DOCTYPE declarations are not accepted", "TEQ 0995"]);
    let (out, code) = run(&[Path::new("check"), &file]);
    assert_eq!(code, 1, "{out}");
    contains_all(&out, &["TEQ 0995"]);
}

#[test]
fn neither_interface_and_missing_files_are_named() {
    let dir = temp("neither");
    let file = dir.join("other.xml");
    fs::write(&file, "<a xmlns=\"urn:other\"/>").unwrap();
    let (out, code) = run(&[Path::new("read"), &file, &dir.join("missing.xml")]);
    assert_eq!(code, 1, "{out}");
    contains_all(&out, &["neither a WBCI nor an S/PRI payload", "TEQ 0995", "missing.xml", "cannot open"]);
}

#[test]
fn every_fixture_reads() {
    let mut args = vec![PathBuf::from("read")];
    args.extend(all_fixtures());
    let (out, code) = run(&args);
    assert_eq!(code, 0, "{out}");
    assert!(!out.contains("unreadable"), "{out}");
}

// -- check ----------------------------------------------------------------------------------

#[test]
fn check_reports_issues_of_both_interfaces() {
    let (out, code) = run(&[Path::new("check"), &wbci_fixture("va-kue-mrn.xml"), &spri_fixture("auftrag-pv.xml")]);
    assert_eq!(code, 0, "{out}");
    contains_all(&out, &["WBCI: no issues", "S/PRI: no issues"]);
    assert!(!out.contains("readable"), "{out}");
    // A warning does not fail the check.
    let (out, code) = run(&[Path::new("check"), &wbci_fixture("str-auf-ekpabg.xml")]);
    assert_eq!(code, 0, "{out}");
    contains_all(&out, &["warning [str-auf.ekpabg-grund] Anfrage/Stornogrund", "(Anl. 1 V5.4.1 §2.9.2)"]);
}

#[test]
fn every_spri_fixture_checks_without_error() {
    let files = fixtures(&spri_fixture(""));
    assert_eq!(files.len(), 30);
    let mut args = vec![PathBuf::from("check")];
    args.extend(files);
    let (out, code) = run(&args);
    assert_eq!(code, 0, "{out}");
    assert!(!out.contains("error ["), "{out}");
}

/// Writes `payload` as `name` into a fresh folder and runs `check` on it.
fn check_spri(name: &str, payload: &spri::Payload) -> (String, i32) {
    let dir = temp(name);
    let file = dir.join(format!("{name}.xml"));
    fs::write(&file, payload.to_xml().unwrap()).unwrap();
    run(&[Path::new("check"), &file])
}

#[test]
fn check_fails_on_an_spri_error() {
    // A receipt that is OK and an error at once.
    let spri::Payload::AuftragResponse { control, mut quittung } = spri_payload("teq-auftrag.xml") else { panic!() };
    quittung.positionen.push(spri::Position::new(spri::Meldungscode::DATENSTRUKTUR_UNZULAESSIG));
    let (out, code) = check_spri("teq-mixed", &spri::Payload::AuftragResponse { control, quittung });
    assert_eq!(code, 1, "{out}");
    contains_all(&out, &["error [spri.teq.mixed] quittung/meldungspositionen", "(S/PRI 4.4 §11.2.3.1)"]);
    // A Terminverschiebung of a feature change: the network owner answers 1007.
    let mut p = spri_payload("auftrag-aen-lmae.xml");
    if let spri::Payload::AuftragRequest { auftrag, .. } = &mut p {
        auftrag.aenderungskennzeichen = spri::Aenderungskennzeichen::Terminverschiebung;
    }
    let (out, code) = check_spri("aen-lmae-tv", &p);
    assert_eq!(code, 1, "{out}");
    contains_all(&out, &["error [spri.aekz.terminverschiebung] auftrag/aenderungskennzeichen", "→ 1007"]);
}

#[test]
fn an_spri_warning_does_not_fail_the_check() {
    let mut p = spri_payload("auftrag-lae.xml");
    if let spri::Payload::AuftragRequest { auftrag, .. } = &mut p {
        auftrag.termin_neu = Some("2027-01-20".parse().unwrap());
    }
    let (out, code) = check_spri("lae-termin-neu", &p);
    assert_eq!(code, 0, "{out}");
    contains_all(&out, &["warning [spri.termin-neu.only-tv] auftrag/terminNeu"]);
}

#[test]
fn spri_issues_print_no_control_characters() {
    // spri-validation/SR-003
    let dir = temp("spri-escape");
    let xml = fs::read_to_string(spri_fixture("meldung-abbm.xml")).unwrap();
    assert!(xml.contains("<meldungscode>1001</meldungscode>"));
    let evil = xml.replace("<meldungscode>1001</meldungscode>", "<meldungscode>\u{1b}[2J</meldungscode>");
    let file = dir.join("abbm-esc.xml");
    fs::write(&file, &evil).unwrap();
    let (out, code) = run(&[Path::new("check"), &file]);
    assert_eq!(code, 0, "an unknown code is a warning: {out}");
    contains_all(&out, &["warning [spri.code.unknown]"]);
    assert!(!out.contains('\u{1b}'), "{out:?}");
    #[cfg(unix)]
    {
        let named = dir.join("evil\u{1b}[2J.xml");
        fs::write(&named, &evil).unwrap();
        let (out, _) = run(&[Path::new("check"), &named]);
        assert!(!out.contains('\u{1b}') && out.contains("evil\u{FFFD}[2J.xml"), "{out:?}");
    }
}

#[test]
fn check_fails_on_an_error() {
    // An ABBM code in a RUEM-VA: the code does not apply to the message type.
    let dir = temp("check-error");
    let file = dir.join("ruem-va-8103.xml");
    let xml = fs::read_to_string(wbci_fixture("ruem-va.xml")).unwrap();
    assert!(xml.contains("<Meldungscode>8001</Meldungscode>"));
    fs::write(&file, xml.replace("<Meldungscode>8001</Meldungscode>", "<Meldungscode>8103</Meldungscode>")).unwrap();
    let (out, code) = run(&[Path::new("check"), &file]);
    assert_eq!(code, 1, "{out}");
    contains_all(&out, &["error [", "Meldungscode", "8103"]);
}

// -- code -----------------------------------------------------------------------------------

#[test]
fn code_explains_both_interfaces() {
    let (out, code) = run(&["code", "8103"]);
    assert_eq!(code, 0, "{out}");
    contains_all(
        &out,
        &[
            "WBCI 8103 Rufnummer nicht geschaltet (RNG)",
            "short name: RNG",
            "ABBM in VA-KUE-MRN, VA-KUE-ORN, VA-RRNP",
            "S/PRI: 8103 is not an S/PRI code",
        ],
    );
    let (out, code) = run(&["code", "0021", "--spri"]);
    assert_eq!(code, 0, "{out}");
    contains_all(
        &out,
        &["S/PRI 0021 Antwort des abgebenden Providers.", "message types: RUEM-PV", "business cases: PV"],
    );
    assert!(!out.contains("WBCI"), "{out}");
    let (out, code) = run(&["code", "OK"]);
    assert_eq!(code, 0, "{out}");
    contains_all(&out, &["WBCI OK OK.", "S/PRI OK OK."]);
}

#[test]
fn an_unknown_code_fails() {
    let (out, code) = run(&["code", "0021", "--wbci"]);
    assert_eq!(code, 1, "{out}");
    contains_all(&out, &["WBCI: 0021 is not a WBCI code"]);
    let (_, code) = run(&["code", "1", "--wbci", "--spri"]);
    assert_eq!(code, 2, "usage error");
}

// -- timeline -------------------------------------------------------------------------------

const ID: &str = "DEU.EKP1.V000000013";
const PV_LINE: &str = "DEU.NB01.1754639821";

fn at(s: &str) -> wbci::Timestamp {
    format!("{s}+01:00").parse().unwrap()
}

fn wbci_payload(name: &str) -> wbci::Payload {
    wbci::Payload::from_xml(&fs::read(wbci_fixture(name)).unwrap()).unwrap()
}

fn spri_payload(name: &str) -> spri::Payload {
    spri::Payload::from_xml(&fs::read(spri_fixture(name)).unwrap()).unwrap()
}

/// A WBCI fixture payload sent at `when`.
fn wbci_at(mut p: wbci::Payload, when: &str) -> wbci::Payload {
    let z = wbci::Zeitstempel::berlin(at(when));
    match &mut p {
        wbci::Payload::AnfrageRequest { control, .. }
        | wbci::Payload::AnfrageResponse { control, .. }
        | wbci::Payload::MeldungRequest { control, .. }
        | wbci::Payload::MeldungResponse { control, .. } => control.zeitstempel = z,
    }
    p
}

fn spri_at(mut p: spri::Payload, when: &str) -> spri::Payload {
    let z = spri::Zeitstempel::berlin(at(when));
    match &mut p {
        spri::Payload::AuftragRequest { control, .. }
        | spri::Payload::AuftragResponse { control, .. }
        | spri::Payload::MeldungRequest { control, .. }
        | spri::Payload::MeldungResponse { control, .. } => control.zeitstempel = z,
    }
    p
}

/// The positive WBCI receipt for a message, sent by `absender` at `when`.
fn wbci_teq(absender: &str, when: &str) -> wbci::Payload {
    let control = wbci_payload("teq-meldung.xml").control().clone();
    let quittung = wbci::Teq {
        absender: absender.parse().unwrap(),
        vorabstimmungs_id: ID.parse().unwrap(),
        positionen: vec![wbci::Position::new(wbci::Meldungscode::Ok)],
        storno_id: None,
        aenderungs_id: None,
    };
    wbci_at(wbci::Payload::MeldungResponse { control, quittung }, when)
}

/// An S/PRI message fixture as a message of business case PV.
fn spri_pv_message(name: &str, when: &str, edit: impl FnOnce(&mut spri::Meldung)) -> spri::Payload {
    let mut p = spri_payload(name);
    if let spri::Payload::MeldungRequest { meldung, .. } = &mut p {
        meldung.auftragstyp.geschaeftsfall = spri::GeschaeftsfallCode::Pv;
        edit(meldung);
    }
    spri_at(p, when)
}

/// The positive S/PRI receipt for a message of order A-2026-0001.
fn spri_teq(when: &str) -> spri::Payload {
    let spri::Payload::AuftragResponse { control, .. } = spri_payload("teq-auftrag.xml") else { panic!() };
    let quittung = spri::Teq::new(
        spri::Meldungscode::Ok,
        Some(spri::ExterneAuftragsnummer::new("A-2026-0001").unwrap()),
        Some(spri::Auftraggebernummer::new("4711").unwrap()),
    );
    spri_at(spri::Payload::MeldungResponse { control, quittung }, when)
}

/// The files of one switch, as the gaining provider captured them: WBCI VA, RUEM-VA and
/// AKM-TR, S/PRI PV (requested for `pv_date`), QEB and ABM, each with its receipt. The PV is
/// SOAP-wrapped; file names do not follow the time order.
fn write_switch(dir: &Path, pv_date: &str) {
    let write = |name: &str, xml: String| fs::write(dir.join(name), xml).unwrap();
    write("01-va.xml", wbci_at(wbci_payload("va-kue-mrn.xml"), "2026-11-23T09:00:00").to_xml().unwrap());
    write("02-teq.xml", wbci_at(wbci_payload("teq-anfrage.xml"), "2026-11-23T09:00:05").to_xml().unwrap());
    write("03-ruem-va.xml", wbci_at(wbci_payload("ruem-va.xml"), "2026-11-24T10:00:00").to_xml().unwrap());
    write("04-teq.xml", wbci_teq("DEU.EKP1", "2026-11-24T10:00:05").to_xml().unwrap());
    let mut akm = wbci_payload("akm-tr.xml");
    if let wbci::Payload::MeldungRequest { meldung: wbci::Meldung::AkmTr(t), .. } = &mut akm {
        t.uebernahme_leitung = vec![wbci::UebernahmeLeitung::LineId(PV_LINE.parse().unwrap())];
    }
    write("05-akm-tr.xml", wbci_at(akm, "2026-11-25T08:00:00").to_xml().unwrap());
    write("06-teq.xml", wbci_teq("DEU.EKP2", "2026-11-25T08:00:05").to_xml().unwrap());

    let mut pv = spri_payload("auftrag-pv.xml");
    if let spri::Payload::AuftragRequest { auftrag, .. } = &mut pv
        && let spri::Geschaeftsfall::Pv(p) = &mut auftrag.geschaeftsfall
    {
        p.wunschtermin.datum = pv_date.parse().unwrap();
    }
    write("a-pv.xml", spri_at(pv, "2026-11-26T09:00:00").to_soap().unwrap());
    write("b-teq.xml", spri_at(spri_payload("teq-auftrag.xml"), "2026-11-26T09:00:05").to_xml().unwrap());
    write("c-qeb.xml", spri_pv_message("meldung-qeb.xml", "2026-11-26T09:05:00", |_| {}).to_xml().unwrap());
    write("d-teq.xml", spri_teq("2026-11-26T09:05:05").to_xml().unwrap());
    let abm = spri_pv_message("meldung-abm.xml", "2026-11-27T11:00:00", |m| {
        if let spri::Meldungsinhalt::Abm(a) = &mut m.inhalt {
            a.verbindlicher_liefertermin.datum = "2027-01-15".parse().unwrap();
        }
    });
    write("e-abm.xml", abm.to_xml().unwrap());
    write("f-teq.xml", spri_teq("2026-11-27T11:00:05").to_xml().unwrap());
}

/// The lines of the output's timeline section.
fn timeline_lines(out: &str) -> Vec<&str> {
    out.lines().skip_while(|l| *l != "Timeline:").skip(1).take_while(|l| l.starts_with("  ")).map(str::trim).collect()
}

#[test]
fn a_consistent_switch_replays_without_findings() {
    let dir = temp("switch");
    write_switch(&dir, "2027-01-15");
    let (out, code) = run(&[Path::new("timeline"), &dir, Path::new("--as"), Path::new("ekpauf")]);
    assert_eq!(code, 0, "{out}");
    contains_all(
        &out,
        &[&format!("Switch {ID}, replayed as EKPauf"), "(the latest control timestamp)", "Findings: none"],
    );
    let lines = timeline_lines(&out);
    assert_eq!(
        lines,
        [
            "Mo 23.11.2026 09:00 WBCI EKPauf → VA-KUE-MRN",
            "Mo 23.11.2026 09:00 WBCI EKPabg → TEQ OK",
            "Di 24.11.2026 10:00 WBCI EKPabg → RUEM-VA 8001 8104",
            "Di 24.11.2026 10:00 WBCI EKPauf → TEQ OK",
            "Mi 25.11.2026 08:00 WBCI EKPauf → AKM-TR 8010",
            "Mi 25.11.2026 08:00 WBCI EKPabg → TEQ OK",
            "Do 26.11.2026 09:00 S/PRI EKPauf → PV",
            "Do 26.11.2026 09:00 S/PRI Netzbetreiber → TEQ OK",
            "Do 26.11.2026 09:05 S/PRI Netzbetreiber → QEB 0000",
            "Do 26.11.2026 09:05 S/PRI EKPauf → TEQ OK",
            "Fr 27.11.2026 11:00 S/PRI Netzbetreiber → ABM 0000 0002",
            "Fr 27.11.2026 11:00 S/PRI EKPauf → TEQ OK",
        ],
        "{out}"
    );
    assert!(!out.contains("OVERDUE") && !out.contains("Refused") && !out.contains("Skipped"), "{out}");
}

#[test]
fn a_pv_date_other_than_the_switch_date_is_a_finding() {
    let dir = temp("switch-termin");
    write_switch(&dir, "2027-01-22");
    let (out, code) = run(&[Path::new("timeline"), &dir, Path::new("--as"), Path::new("ekpauf")]);
    assert_eq!(code, 1, "{out}");
    contains_all(
        &out,
        &[
            "Findings:",
            "wechsel.termin",
            "order A-2026-0001: requested Fr 22.01.2027 — the WBCI switch date is Fr 15.01.2027",
        ],
    );
}

#[test]
fn deadlines_are_judged_at_now() {
    let dir = temp("switch-now");
    write_switch(&dir, "2027-01-15");
    // Without the last receipt the ABM awaits one; a minute later it is overdue.
    fs::remove_file(dir.join("f-teq.xml")).unwrap();
    let (out, code) = run(&[
        Path::new("timeline"),
        &dir,
        Path::new("--as"),
        Path::new("ekpauf"),
        Path::new("--now"),
        Path::new("2026-11-27T11:01:00+01:00"),
    ]);
    assert_eq!(code, 1, "{out}");
    contains_all(&out, &["(--now)", "OVERDUE S/PRI EKPauf"]);
    let (out, code) = run(&[
        Path::new("timeline"),
        &dir,
        Path::new("--as"),
        Path::new("ekpauf"),
        Path::new("--now"),
        Path::new("not a time"),
    ]);
    assert_eq!(code, 2, "{out}");
}

#[test]
fn files_of_other_switches_or_views_are_listed_not_dropped() {
    let dir = temp("switch-others");
    write_switch(&dir, "2027-01-15");
    // The AKM-PV goes from the network owner to the losing provider: not the EKPauf's view.
    let akm = spri_pv_message("meldung-akm-pv.xml", "2026-11-26T10:00:00", |_| {});
    fs::write(dir.join("g-akm-pv.xml"), akm.to_xml().unwrap()).unwrap();
    // A pre-agreement of another switch.
    let mut other = wbci_payload("va-kue-orn.xml");
    if let wbci::Payload::AnfrageRequest { anfrage: wbci::Anfrage::Vorabstimmung(va), .. } = &mut other {
        va.id = "DEU.EKP1.V000000099".parse().unwrap();
    }
    fs::write(dir.join("h-va.xml"), wbci_at(other, "2026-11-28T09:00:00").to_xml().unwrap()).unwrap();
    // An ERLM before its time: the order is only confirmed, not yet due — refused or not,
    // it is named.
    fs::write(dir.join("i-broken.xml"), "<not-xml").unwrap();
    let (out, code) = run(&[Path::new("timeline"), &dir, Path::new("--as"), Path::new("ekpauf")]);
    assert_eq!(code, 1, "the unreadable file fails the replay:\n{out}");
    contains_all(
        &out,
        &[
            "Skipped:",
            "g-akm-pv.xml — S/PRI: not in the EKPauf's view",
            "h-va.xml — WBCI: Vorabstimmungs-ID DEU.EKP1.V000000099, another switch",
            "Unreadable:",
            "i-broken.xml",
        ],
    );
}

/// The leg between the network owner and the losing provider: AKM-PV, RUEM-PV and ABM-PV,
/// each with its receipt.
fn write_pv_leg(dir: &Path) {
    let write = |name: &str, p: spri::Payload| fs::write(dir.join(name), p.to_xml().unwrap()).unwrap();
    write("g-akm-pv.xml", spri_pv_message("meldung-akm-pv.xml", "2026-11-26T10:00:00", |_| {}));
    write("g-teq.xml", spri_teq("2026-11-26T10:00:05"));
    write("h-ruem-pv.xml", spri_pv_message("meldung-ruem-pv.xml", "2026-11-27T10:00:00", |_| {}));
    write("h-teq.xml", spri_teq("2026-11-27T10:00:05"));
    write("i-abm-pv.xml", spri_pv_message("meldung-abm-pv.xml", "2026-11-27T11:01:00", |_| {}));
    write("i-teq.xml", spri_teq("2026-11-27T11:01:05"));
}

#[test]
fn the_network_owner_replays_spri_alone() {
    let dir = temp("switch-nb");
    write_switch(&dir, "2027-01-15");
    write_pv_leg(&dir);
    let (out, code) = run(&[Path::new("timeline"), &dir, Path::new("--as"), Path::new("netzbetreiber")]);
    assert_eq!(code, 0, "{out}");
    contains_all(
        &out,
        &[
            "replayed as Netzbetreiber",
            "Do 26.11.2026 09:00 S/PRI EKPauf → PV",
            "Do 26.11.2026 10:00 S/PRI Netzbetreiber → AKM-PV",
            "Fr 27.11.2026 10:00 S/PRI EKPabg → RUEM-PV 0021",
            "Fr 27.11.2026 11:00 S/PRI Netzbetreiber → ABM 0000 0002",
            "Fr 27.11.2026 11:01 S/PRI Netzbetreiber → ABM-PV",
            "the network owner sees no WBCI",
            "01-va.xml — WBCI: the network owner is no WBCI party",
        ],
    );
    assert!(!out.contains("Refused"), "{out}");
}

#[test]
fn the_losing_provider_replays_its_view() {
    let dir = temp("switch-abg");
    write_switch(&dir, "2027-01-15");
    write_pv_leg(&dir);
    let (out, code) = run(&[Path::new("timeline"), &dir, Path::new("--as"), Path::new("ekpabg")]);
    assert_eq!(code, 0, "{out}");
    contains_all(
        &out,
        &[
            "replayed as EKPabg",
            "Di 24.11.2026 10:00 WBCI EKPabg → RUEM-VA 8001 8104",
            "Do 26.11.2026 10:00 S/PRI Netzbetreiber → AKM-PV",
            "Fr 27.11.2026 10:00 S/PRI EKPabg → RUEM-PV 0021",
            "Findings: none",
            "a-pv.xml — S/PRI: order A-2026-0001 is placed by the EKPauf",
            "c-qeb.xml — S/PRI: not in the EKPabg's view",
        ],
    );
    assert!(!out.contains("Refused"), "{out}");
}

#[test]
fn an_empty_folder_fails() {
    let dir = temp("empty");
    let (out, code) = run(&[Path::new("timeline"), &dir, Path::new("--as"), Path::new("ekpabg")]);
    assert_eq!(code, 1, "{out}");
    contains_all(&out, &["No switch"]);
}

// -- security -------------------------------------------------------------------------------

#[test]
fn control_characters_from_input_are_replaced() {
    // cli/SR-001
    let dir = temp("escape");
    let file = dir.join("abbm-esc.xml");
    let xml = fs::read_to_string(wbci_fixture("abbm.xml")).unwrap();
    assert!(xml.contains("<Meldungscode>8103</Meldungscode>"));
    fs::write(&file, xml.replace("<Meldungscode>8103</Meldungscode>", "<Meldungscode>\u{1b}[2J</Meldungscode>"))
        .unwrap();
    // The reader keeps an unknown code as it is.
    let p = wbci::Payload::from_xml(&fs::read(&file).unwrap()).unwrap();
    let wbci::Payload::MeldungRequest { meldung, .. } = &p else { panic!() };
    assert_eq!(meldung.codes().next().unwrap().as_str(), "\u{1b}[2J");
    for cmd in ["read", "check"] {
        let (out, _) = run(&[Path::new(cmd), &file]);
        assert!(!out.contains('\u{1b}'), "{cmd}: {out:?}");
        assert!(out.contains("\u{FFFD}[2J"), "{cmd}: {out:?}");
    }
    let (out, code) = run(&["code", "\u{1b}[2J"]);
    assert_eq!(code, 1);
    assert!(!out.contains('\u{1b}') && out.contains('\u{FFFD}'), "{out:?}");
    // A file name with a control character is printed safely too.
    #[cfg(unix)]
    {
        let named = dir.join("evil\u{1b}[2J.xml");
        fs::write(&named, xml).unwrap();
        let (out, _) = run(&[Path::new("timeline"), &dir, Path::new("--as"), Path::new("ekpauf")]);
        assert!(!out.contains('\u{1b}'), "{out:?}");
        assert!(out.contains("evil\u{FFFD}[2J.xml"), "{out:?}");
    }
}

#[test]
fn no_personal_data_in_any_output() {
    // cli/SR-002
    let dir = temp("pii");
    write_switch(&dir, "2027-01-22");
    let mut outputs = String::new();
    for cmd in ["read", "check"] {
        let mut args = vec![PathBuf::from(cmd)];
        args.extend(all_fixtures());
        args.extend(fixtures(&dir));
        outputs.push_str(&run(&args).0);
    }
    for party in ["ekpauf", "ekpabg", "netzbetreiber"] {
        outputs.push_str(&run(&[Path::new("timeline"), &dir, Path::new("--as"), Path::new(party)]).0);
        outputs.push_str(&run(&[Path::new("timeline"), &spri_fixture(""), Path::new("--as"), Path::new(party)]).0);
        outputs.push_str(&run(&[Path::new("timeline"), &wbci_fixture(""), Path::new("--as"), Path::new(party)]).0);
    }
    assert!(outputs.contains("Findings:") && outputs.contains("interface: S/PRI"), "{outputs}");
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
}

#[test]
fn a_replayed_folder_skips_links_and_sub_folders() {
    // cli/SR-003
    let dir = temp("links");
    write_switch(&dir, "2027-01-15");
    let sub = dir.join("nested");
    fs::create_dir(&sub).unwrap();
    fs::copy(wbci_fixture("abbm.xml"), sub.join("abbm.xml")).unwrap();
    #[cfg(unix)]
    std::os::unix::fs::symlink(wbci_fixture("abbm.xml"), dir.join("link.xml")).unwrap();
    let (out, code) = run(&[Path::new("timeline"), &dir, Path::new("--as"), Path::new("ekpauf")]);
    assert_eq!(code, 0, "skipping is not a failure:\n{out}");
    contains_all(&out, &["Skipped:", "nested — sub-folder, not searched"]);
    #[cfg(unix)]
    contains_all(&out, &["link.xml — symbolic link, not followed"]);
    // The file inside the sub-folder and behind the link were not read: the ABBM is not in
    // the timeline.
    assert!(!out.contains("ABBM"), "{out}");
}

#[test]
fn a_folder_above_the_limit_is_refused() {
    // cli/SR-003
    let dir = temp("many");
    for i in 0..=10_000 {
        fs::write(dir.join(format!("{i:05}.xml")), "").unwrap();
    }
    let (out, code) = run(&[Path::new("timeline"), &dir, Path::new("--as"), Path::new("ekpauf")]);
    assert_eq!(code, 1, "{out}");
    contains_all(&out, &["error: more than 10000 files"]);
}

#[test]
fn check_judges_dates_at_the_files_own_timestamp() {
    let dir = temp("sent-at");
    // S/PRI: a wish date before the order's control timestamp is refused (an error, exit 1).
    let pv = fs::read_to_string(spri_fixture("auftrag-pv.xml")).unwrap();
    assert!(pv.contains("<datum>2027-01-15</datum>"));
    let late = dir.join("pv-past.xml");
    fs::write(&late, pv.replace("<datum>2027-01-15</datum>", "<datum>2026-11-20</datum>")).unwrap();
    let (out, code) = run(&[Path::new("check"), &late]);
    assert_eq!(code, 1, "{out}");
    contains_all(&out, &["spri.termin.past"]);
    // WBCI: a Kundenwunschtermin closer than the lead time allows is reported.
    let va = fs::read_to_string(wbci_fixture("va-kue-mrn.xml")).unwrap();
    assert!(va.contains("<Kundenwunschtermin>2027-01-15</Kundenwunschtermin>"));
    let soon = dir.join("va-soon.xml");
    fs::write(
        &soon,
        va.replace(
            "<Kundenwunschtermin>2027-01-15</Kundenwunschtermin>",
            "<Kundenwunschtermin>2026-11-25</Kundenwunschtermin>",
        ),
    )
    .unwrap();
    let (out, _) = run(&[Path::new("check"), &soon]);
    contains_all(&out, &["warning [va.lead]"]);
    // The unchanged fixtures stay clean at their own timestamps.
    let (out, code) = run(&[Path::new("check"), &spri_fixture("auftrag-pv.xml"), &wbci_fixture("va-kue-mrn.xml")]);
    assert_eq!(code, 0, "{out}");
}

// -- simulate -------------------------------------------------------------------------------

fn names() -> Vec<&'static str> {
    telco_interop::sim::katalog().iter().map(|s| s.name).collect()
}

#[test]
fn simulate_lists_the_catalogue() {
    let (out, code) = run(&["simulate", "--list"]);
    assert_eq!(code, 0, "{out}");
    assert_eq!(out.lines().count(), names().len(), "{out}");
    for (line, s) in out.lines().zip(telco_interop::sim::katalog()) {
        assert_eq!(line, format!("{} — {} · {}", s.name, s.beschreibung, s.quelle));
    }
    let (out, code) = run(&["simulate", "--list", "spri-pv"]);
    assert_eq!(code, 2, "--list takes no scenario: {out}");
    let (out, code) = run(&["simulate", "spri-pv"]);
    assert_eq!(code, 2, "a folder is required: {out}");
}

#[test]
fn simulate_writes_files_in_time_order_and_never_overwrites() {
    // sandbox/SR-006
    let dir = temp("simulate");
    let target = dir.join("pv");
    let (out, code) = run(&[Path::new("simulate"), Path::new("spri-pv"), &target]);
    assert_eq!(code, 0, "{out}");
    contains_all(&out, &["wrote ", "001-spri-ekpauf-pv.xml", "20 files of scenario spri-pv", "telco-interop timeline"]);
    let files = fixtures(&target);
    assert_eq!(files.len(), 20);
    let first = fs::read_to_string(&files[0]).unwrap();
    assert!(first.contains("PV") || first.contains("ProviderwechselType"), "{first}");

    // A second run without --force refuses and changes nothing.
    fs::write(&files[0], "mine").unwrap();
    let (out, code) = run(&[Path::new("simulate"), Path::new("spri-pv"), &target]);
    assert_eq!(code, 1, "{out}");
    contains_all(&out, &["error: ", "001-spri-ekpauf-pv.xml exists (use --force"]);
    assert_eq!(fs::read_to_string(&files[0]).unwrap(), "mine");
    assert_eq!(fixtures(&target).len(), 20);

    // --force replaces them.
    let (out, code) = run(&[Path::new("simulate"), Path::new("spri-pv"), &target, Path::new("--force")]);
    assert_eq!(code, 0, "{out}");
    assert_eq!(fs::read_to_string(&files[0]).unwrap(), first);

    // An unknown scenario points to the list.
    let (out, code) = run(&[Path::new("simulate"), Path::new("nope"), &target]);
    assert_eq!(code, 1, "{out}");
    contains_all(&out, &["no scenario nope (see simulate --list)"]);

    // A folder below a regular file cannot be created.
    let file = dir.join("plain.txt");
    fs::write(&file, "x").unwrap();
    let (out, code) = run(&[Path::new("simulate"), Path::new("spri-neu"), &file.join("sub")]);
    assert_eq!(code, 1, "{out}");
    contains_all(&out, &["cannot create the folder"]);
}

#[cfg(unix)]
#[test]
fn simulate_never_writes_through_a_link() {
    // sandbox/SR-006
    let dir = temp("simulate-link");
    let outside = dir.join("outside.txt");
    fs::write(&outside, "keep").unwrap();
    let target = dir.join("neu");
    fs::create_dir(&target).unwrap();
    let link = target.join("001-spri-ekpauf-neu.xml");
    std::os::unix::fs::symlink(&outside, &link).unwrap();
    let (out, code) = run(&[Path::new("simulate"), Path::new("spri-neu"), &target]);
    assert_eq!(code, 1, "a link is an existing entry: {out}");
    let (out, code) = run(&[Path::new("simulate"), Path::new("spri-neu"), &target, Path::new("--force")]);
    assert_eq!(code, 0, "{out}");
    assert_eq!(fs::read_to_string(&outside).unwrap(), "keep");
    assert!(fs::symlink_metadata(&link).unwrap().is_file(), "the link was replaced by a file");
}

#[cfg(unix)]
#[test]
fn simulate_prints_no_control_characters() {
    // sandbox/SR-007
    let dir = temp("simulate-escape");
    let target = dir.join("evil\u{1b}[2J");
    let (out, code) = run(&[Path::new("simulate"), Path::new("wbci-agreed"), &target]);
    assert_eq!(code, 0, "{out}");
    assert!(!out.contains('\u{1b}') && out.contains("evil\u{FFFD}[2J"), "{out:?}");
}

#[test]
fn every_scenario_replays_for_every_party() {
    // sandbox/SR-004
    let dir = temp("simulate-all");
    for name in names() {
        let target = dir.join(name);
        let (out, code) = run(&[Path::new("simulate"), Path::new(name), &target]);
        assert_eq!(code, 0, "{name}: {out}");
        for party in ["ekpauf", "ekpabg", "netzbetreiber"] {
            let (out, code) = run(&[Path::new("timeline"), &target, Path::new("--as"), Path::new(party)]);
            assert_eq!(code, 0, "{name} as {party}:\n{out}");
            assert!(out.contains("Findings: none"), "{name} as {party}:\n{out}");
            assert!(
                !out.contains("Refused") && !out.contains("OVERDUE") && !out.contains("Unreadable"),
                "{name} as {party}:\n{out}"
            );
            // Only a party that takes no part in the scenario's interface sees nothing.
            let blind = (name.starts_with("wbci-") && party == "netzbetreiber")
                || (name.starts_with("spri-neu") || name.starts_with("spri-storno") || name == "spri-tv")
                    && party == "ekpabg";
            assert_eq!(timeline_lines(&out) == ["(empty)"], blind, "{name} as {party}:\n{out}");
        }
    }
}

#[test]
fn simulate_starts_on_a_date_or_at_a_timestamp() {
    let dir = temp("sim-start");
    let (out, code) = run(&[
        Path::new("simulate"),
        Path::new("wechsel"),
        &dir.join("date"),
        Path::new("--start"),
        Path::new("2027-12-23"),
    ]);
    assert_eq!(code, 0, "{out}");
    let (out, code) = run(&[Path::new("timeline"), &dir.join("date"), Path::new("--as"), Path::new("ekpauf")]);
    assert_eq!(code, 0, "{out}");
    contains_all(&out, &["Do 23.12.2027 09:00 WBCI EKPauf → VA-KUE-MRN", "Findings: none"]);
    let (out, code) = run(&[
        Path::new("simulate"),
        Path::new("wechsel"),
        &dir.join("ts"),
        Path::new("--start"),
        Path::new("2027-12-23T08:00:00Z"),
    ]);
    assert_eq!(code, 0, "{out}");
    let (out, code) =
        run(&[Path::new("simulate"), Path::new("wechsel"), &dir.join("bad"), Path::new("--start"), Path::new("nope")]);
    assert_eq!(code, 2, "{out}");
    assert!(!dir.join("bad").exists());
}
