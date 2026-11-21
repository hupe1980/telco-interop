//! The losing provider's check against every row of Anl. 1 V5.4.1 §2.4 and every code of §2.3.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::indexing_slicing)]

mod common;

use common::*;
use proptest::prelude::*;
use wbci::pruefung::{self, Anlage, Bestand, Ergebnis, Feld, Methode, Pruefung};
use wbci::*;

fn received() -> Timestamp {
    ts("2026-11-23T08:00:00Z")
}

fn record() -> Bestand {
    let mut b = Bestand::new(kunde(), "D123".parse().unwrap());
    b.standort = Some(standort());
    b.rufnummern = vec![nummer("30", "1234567"), nummer("30", "1234568")];
    b.ressourcen =
        vec![TechnischeRessource { line_id: Some("DEU.EKP2.1754639821".parse().unwrap()), ..Default::default() }];
    b.technologie = Some("001 TAL ISDN".parse().unwrap());
    b
}

fn run(va: &Vorabstimmung, b: &Bestand) -> Pruefung {
    let p = pruefung::check(va, b, received(), &Calendar::wbci());
    let issues = p.antwort.issues_at(Some(&va.clone().into()), received(), &Calendar::wbci());
    assert!(issues.iter().all(|i| i.severity == Severity::Warning), "answer must validate clean: {issues:#?}\n{p}");
    p
}

fn codes(p: &Pruefung) -> Vec<&str> {
    p.antwort.codes().map(Meldungscode::as_str).collect()
}

fn set_name(va: &mut Vorabstimmung, vorname: Option<&str>, nachname: &str) {
    va.endkunde =
        Kunde::person(Anrede::Frau, vorname.map(|v| Vorname::new(v).unwrap()), Nachname::new(nachname).unwrap());
}

fn with_numbers(va: &mut Vorabstimmung, numbers: &[(&str, &str)], alle: bool) {
    if let VorabstimmungsFall::KueMrn { rufnummernportierung, .. } = &mut va.fall {
        rufnummernportierung.anschluss = Anschluss::Einzel {
            rufnummern: numbers.iter().map(|(o, n)| nummer(o, n)).collect(),
            alle_rufnummern: alle,
        };
    }
}

fn set_standort(va: &mut Vorabstimmung, f: impl FnOnce(&mut Standort)) {
    match &mut va.fall {
        VorabstimmungsFall::KueMrn { standort, .. } | VorabstimmungsFall::KueOrn { standort, .. } => f(standort),
        VorabstimmungsFall::Rrnp { .. } => {}
    }
}

#[test]
fn everything_matches_zwa_with_resources_and_all_numbers() {
    let p = run(&va_kue_mrn(), &record());
    assert!(p.is_agreed());
    assert_eq!(codes(&p), ["8001"]);
    let Meldung::RuemVa(r) = &p.antwort else { panic!() };
    assert_eq!(r.wechseltermin, date("2027-01-15"));
    assert_eq!(r.ressourcen.len(), 1);
    assert!(r.technologie.is_some());
    // „alle Rufnummern": all of the record's numbers are returned with PKIabg.
    let Some(PortierteRufnummern::Rufnummern(list)) = &r.rufnummernportierung else { panic!() };
    assert_eq!(list.len(), 2);
    assert!(list.iter().all(|(_, pki)| pki.as_str() == "D123"));
}

#[test]
fn later_contract_end_gives_nat_moved_to_a_working_day() {
    let mut b = record();
    b.vertragsende = Some(date("2027-01-30")); // Saturday
    let p = run(&va_kue_mrn(), &b);
    assert_eq!(codes(&p), ["8002"]);
    assert_eq!(p.antwort.wechseltermin(), Some(date("2027-02-01")));
    assert!(p.befunde.iter().any(|f| f.feld == Feld::Vertragsende && f.code == Some(Meldungscode::Nat)));
}

#[test]
fn requested_date_too_close_gives_nat_after_15_working_days() {
    let mut va = va_kue_mrn();
    va.kundenwunschtermin = date("2026-11-30");
    let p = run(&va, &record());
    assert_eq!(codes(&p), ["8002"]);
    assert_eq!(p.antwort.wechseltermin(), Some(date("2026-12-14")));
}

#[test]
fn each_address_mismatch_is_an_ada_hint_when_the_number_identifies() {
    type Case = (&'static str, fn(&mut Standort), &'static str);
    let cases: [Case; 4] = [
        ("8104", |s| s.postleitzahl = "10117".parse().unwrap(), "Postleitzahl"),
        ("8105", |s| s.ort = "Potsdam".parse().unwrap(), "Ort"),
        ("8106", |s| s.strassenname = "Nebenweg".parse().unwrap(), "Strasse"),
        ("8107", |s| s.hausnummer = "14".parse().unwrap(), "Hausnummer"),
    ];
    for (code, change, feld) in cases {
        let mut va = va_kue_mrn();
        set_standort(&mut va, change);
        let p = run(&va, &record());
        assert_eq!(codes(&p), ["8001", code], "{feld}");
        let Meldung::RuemVa(r) = &p.antwort else { panic!() };
        assert!(r.positionen[1].adresse_abweichend.is_some());
    }
}

#[test]
fn phonetic_address_and_ignored_suffix_still_match() {
    let mut va = va_kue_mrn();
    set_standort(&mut va, |s| {
        s.strassenname = "Hauptstr.".parse().unwrap();
        s.ort = "berlin".parse().unwrap();
        s.hausnummern_zusatz = Some("b".parse().unwrap());
    });
    assert_eq!(codes(&run(&va, &record())), ["8001"]);
}

#[test]
fn each_address_mismatch_is_adf_when_no_number_identifies() {
    let mut va = va_kue_mrn();
    with_numbers(&mut va, &[("30", "9999999")], true);
    set_standort(&mut va, |s| s.postleitzahl = "10117".parse().unwrap());
    let p = run(&va, &record());
    assert_eq!(codes(&p), ["8103", "8108"]);
}

#[test]
fn each_address_field_has_its_adf_code() {
    type Case = (&'static str, fn(&mut Standort));
    let cases: [Case; 4] = [
        ("8108", |s| s.postleitzahl = "10117".parse().unwrap()),
        ("8109", |s| s.ort = "Potsdam".parse().unwrap()),
        ("8110", |s| s.strassenname = "Nebenweg".parse().unwrap()),
        ("8111", |s| s.hausnummer = "14".parse().unwrap()),
    ];
    for (code, change) in cases {
        let mut va = va_kue_mrn();
        with_numbers(&mut va, &[("30", "9999999")], true);
        set_standort(&mut va, change);
        assert_eq!(codes(&run(&va, &record())), ["8103", code]);
    }
}

#[test]
fn names_by_sound_double_names_and_several_first_names() {
    let mut b = record();
    b.inhaber =
        vec![Kunde::person(Anrede::Frau, Some("Anna Marie".parse().unwrap()), "Müller-Lüdenscheidt".parse().unwrap())];
    for (vor, nach) in [(Some("Marie"), "Mueller"), (Some("Anna-Marie"), "Lüdenscheidt"), (Some("anna"), "MÜLLER")] {
        let mut va = va_kue_mrn();
        set_name(&mut va, vor, nach);
        assert_eq!(codes(&run(&va, &b)), ["8001"], "{vor:?} {nach}");
    }
    // Swapped fields still match (Infodokument §5.5).
    let mut va = va_kue_mrn();
    set_name(&mut va, Some("Mustermann"), "Erika");
    assert_eq!(codes(&run(&va, &record())), ["8001"]);
}

// ekpabg-pruefung/SR-001: a different surname is AIF, abbreviated or merged first names are AIFVN.
#[test]
fn near_miss_names_are_refused() {
    let mut va = va_kue_mrn();
    let mut b = record();
    b.inhaber = vec![Kunde::person(Anrede::Herr, Some("Max".parse().unwrap()), "Schneider".parse().unwrap())];
    set_name(&mut va, Some("Max"), "Schmidt");
    let p = run(&va, &b);
    assert!(!p.is_agreed());
    assert_eq!(codes(&p), ["8112"]);

    let mut b = record();
    b.inhaber = vec![Kunde::person(Anrede::Frau, Some("Anna Marie".parse().unwrap()), "Mustermann".parse().unwrap())];
    for vor in ["A.", "AnnaMarie", "Erika"] {
        set_name(&mut va, Some(vor), "Mustermann");
        assert_eq!(codes(&run(&va, &b)), ["8113"], "{vor}");
    }
    set_name(&mut va, None, "Mustermann");
    assert_eq!(codes(&run(&va, &b)), ["8113"], "a missing first name");
}

#[test]
fn companies_by_their_words() {
    let mut b = record();
    b.inhaber = vec![Kunde::firma("Beispiel GmbH".parse().unwrap())];
    let mut va = va_kue_mrn();
    va.endkunde = Kunde::firma("Beispiel G.m.b.H. & Co. KG".parse().unwrap());
    assert_eq!(codes(&run(&va, &b)), ["8001"]);
    b.inhaber = vec![Kunde::firma("Syscomp Dienste".parse().unwrap())];
    va.endkunde = Kunde::firma("Syscomp Dienstleistungen".parse().unwrap());
    assert_eq!(codes(&run(&va, &b)), ["8112"]);
}

#[test]
fn further_holders_when_the_contract_needs_them() {
    let mut b = record();
    b.alle_inhaber_noetig = true;
    b.inhaber.push(Kunde::person(Anrede::Herr, Some("Max".parse().unwrap()), "Beispiel".parse().unwrap()));
    let p = run(&va_kue_mrn(), &b);
    assert_eq!(codes(&p), ["8114"]);
    let mut va = va_kue_mrn();
    va.weitere_anschlussinhaber =
        vec![Kunde::person(Anrede::Herr, Some("Max".parse().unwrap()), "Beispiel".parse().unwrap())];
    assert_eq!(codes(&run(&va, &b)), ["8001"]);
}

#[test]
fn numbers_with_and_without_alle_rufnummern() {
    // „alle": one right number suffices.
    let mut va = va_kue_mrn();
    with_numbers(&mut va, &[("30", "1234567"), ("30", "5550000")], true);
    assert_eq!(codes(&run(&va, &record())), ["8001"]);
    // Without „alle": a wrong number refuses, naming the record's numbers.
    with_numbers(&mut va, &[("30", "1234567"), ("30", "5550000")], false);
    let p = run(&va, &record());
    assert_eq!(codes(&p), ["8103"]);
    let Meldung::Abbm(a) = &p.antwort else { panic!() };
    assert_eq!(a.positionen[0].rufnummern.len(), 2);
    // None right: refused without numbers.
    with_numbers(&mut va, &[("30", "5550000")], false);
    let Meldung::Abbm(a) = &run(&va, &record()).antwort else { panic!() };
    assert!(a.positionen[0].rufnummern.is_empty());
    // Without „alle", all right: only the given numbers are ported.
    with_numbers(&mut va, &[("30", "1234568")], false);
    let p = run(&va, &record());
    let Meldung::RuemVa(r) = &p.antwort else { panic!() };
    let Some(PortierteRufnummern::Rufnummern(list)) = &r.rufnummernportierung else { panic!() };
    assert_eq!(list.len(), 1);
}

// ekpabg-pruefung/SR-002: record values only when the customer is identified.
#[test]
fn record_values_only_when_identified() {
    let mut va = va_kue_mrn();
    with_numbers(&mut va, &[("30", "1234567"), ("30", "5550000")], false);
    set_name(&mut va, Some("Max"), "Schmidt");
    let p = run(&va, &record());
    assert_eq!(codes(&p), ["8103", "8112"]);
    let Meldung::Abbm(a) = &p.antwort else { panic!() };
    assert!(a.positionen.iter().all(|x| x.rufnummern.is_empty()), "no numbers to an unidentified requester");

    // VA-KUE-ORN, right name, wrong postcode, no number: ADF, never the record's postcode.
    let mut orn = va_kue_orn();
    if let VorabstimmungsFall::KueOrn { anschlussidentifikation, .. } = &mut orn.fall {
        *anschlussidentifikation = None;
    }
    set_standort(&mut orn, |s| s.postleitzahl = "10117".parse().unwrap());
    let mut b = record();
    b.inhaber = vec![orn.endkunde.clone()];
    let p = run(&orn, &b);
    assert_eq!(codes(&p), ["8108"]);
    let xml =
        Payload::MeldungRequest { control: control("2026-11-23T09:00:00Z"), meldung: p.antwort }.to_xml().unwrap();
    assert!(!xml.contains("10115"), "the record's postcode must not be disclosed");
}

#[test]
fn kue_orn_identified_by_location_or_number() {
    let orn = va_kue_orn();
    let mut b = record();
    b.inhaber = vec![orn.endkunde.clone()];
    b.rufnummern = vec![nummer("30", "7654321")];
    assert_eq!(codes(&run(&orn, &b)), ["8001"]);
    // Number identifies, address differs: ADA.
    let mut moved = orn.clone();
    set_standort(&mut moved, |s| s.hausnummer = "99".parse().unwrap());
    assert_eq!(codes(&run(&moved, &b)), ["8001", "8107"]);
    // Wrong identifying number but the address identifies: agreed.
    b.rufnummern = vec![nummer("30", "1111111")];
    assert_eq!(codes(&run(&orn, &b)), ["8001"]);
    // Wrong number and wrong address: RNG and ADF (house number), numbers first (Prüfbaum).
    assert_eq!(codes(&run(&moved, &b)), ["8103", "8111"]);
}

#[test]
fn not_identifiable() {
    // Several lines at the location, no number: KNI.
    let mut orn = va_kue_orn();
    if let VorabstimmungsFall::KueOrn { anschlussidentifikation, .. } = &mut orn.fall {
        *anschlussidentifikation = None;
    }
    let mut b = record();
    b.inhaber = vec![orn.endkunde.clone()];
    b.mehrere_anschluesse = true;
    assert_eq!(codes(&run(&orn, &b)), ["8117"]);
    // Nothing matches at all: KNI alone.
    let mut va = va_kue_mrn();
    with_numbers(&mut va, &[("30", "5550000")], true);
    set_name(&mut va, Some("Max"), "Schmidt");
    set_standort(&mut va, |s| s.postleitzahl = "80331".parse().unwrap());
    assert_eq!(codes(&run(&va, &record())), ["8117"]);
    // No record found.
    assert_eq!(codes(&pruefung::not_found(&va_kue_mrn())), ["8117"]);
}

#[test]
fn other_pre_agreement_and_withdrawal_are_listed_with_other_reasons() {
    let mut b = record();
    b.andere_vorabstimmung = true;
    b.neue_willenserklaerung = true;
    let mut va = va_kue_mrn();
    set_name(&mut va, Some("Lisa"), "Mustermann");
    assert_eq!(codes(&run(&va, &b)), ["8113", "8118", "8150"]);
}

#[test]
fn pbx_blocks_and_the_wrong_block_length() {
    let mut b = record();
    b.rufnummern.clear();
    let VorabstimmungsFall::Rrnp { rufnummernportierung, .. } = va_rrnp().fall else { panic!() };
    let Anschluss::Anlage { kopf, .. } = rufnummernportierung.anschluss.clone() else { panic!() };
    b.anlage = Some(Anlage {
        kopf,
        bloecke: vec![Rufnummernblock { von: "00".parse().unwrap(), bis: "99".parse().unwrap() }],
    });
    let p = run(&va_rrnp(), &b);
    assert_eq!(codes(&p), ["8001"]);
    let Meldung::RuemVa(r) = &p.antwort else { panic!() };
    assert!(matches!(r.rufnummernportierung, Some(PortierteRufnummern::Bloecke { .. })));
    // The request asks for 000–999 where the PBX has 00–99: RNG plus SON naming the right block.
    let mut va = va_rrnp();
    if let VorabstimmungsFall::Rrnp { rufnummernportierung, .. } = &mut va.fall
        && let Anschluss::Anlage { bloecke, .. } = &mut rufnummernportierung.anschluss
    {
        bloecke[0] = Rufnummernblock { von: "000".parse().unwrap(), bis: "999".parse().unwrap() };
    }
    let p = run(&va, &b);
    assert_eq!(codes(&p), ["8103", "8116"]);
    let Meldung::Abbm(a) = &p.antwort else { panic!() };
    assert_eq!(a.begruendung.as_ref().map(Freitext::as_str), Some("korrekter Rufnummernblock: 00 - 99"));
}

#[test]
fn later_porting_within_65_working_days() {
    let mut b = record();
    let VorabstimmungsFall::Rrnp { rufnummernportierung, .. } = va_rrnp().fall else { panic!() };
    let Anschluss::Anlage { kopf, bloecke } = rufnummernportierung.anschluss.clone() else { panic!() };
    b.anlage = Some(Anlage { kopf, bloecke });
    b.abgeschaltet = Some(date("2026-10-01"));
    assert_eq!(codes(&run(&va_rrnp(), &b)), ["8001"]);
    b.abgeschaltet = Some(date("2026-06-01"));
    let p = run(&va_rrnp(), &b);
    assert_eq!(codes(&p), ["8103"]);
    assert!(p.befunde.iter().any(|f| f.feld == Feld::Abschaltung && f.ergebnis == Ergebnis::Mismatch));
}

// ekpabg-pruefung/SR-004: every field compared is recorded with its method and result.
#[test]
fn explanation_lists_every_field() {
    let p = run(&va_kue_mrn(), &record());
    let fields: Vec<(Feld, Methode)> = p.befunde.iter().map(|b| (b.feld, b.methode)).collect();
    for expected in [
        (Feld::Rufnummern, Methode::Exact),
        (Feld::Nachname, Methode::Phonetic),
        (Feld::Vorname, Methode::Phonetic),
        (Feld::Postleitzahl, Methode::Exact),
        (Feld::Ort, Methode::Phonetic),
        (Feld::Strasse, Methode::Phonetic),
        (Feld::Hausnummer, Methode::Exact),
        (Feld::HausnummernZusatz, Methode::NotChecked),
    ] {
        assert!(fields.contains(&expected), "{expected:?} missing: {fields:?}");
    }
    assert!(p.befunde.iter().all(|b| b.ergebnis == Ergebnis::Match || b.methode == Methode::NotChecked));
}

const SECRETS: [&str; 6] = ["Erika", "Mustermann", "Hauptstraße", "1234567", "Schmidt", "10117"];

// ekpabg-pruefung/SR-003: the explanation carries no personal data.
#[test]
fn explanation_holds_no_values() {
    let mut va = va_kue_mrn();
    set_name(&mut va, Some("Erika"), "Schmidt");
    set_standort(&mut va, |s| s.postleitzahl = "10117".parse().unwrap());
    let p = run(&va, &record());
    let text = format!("{p}\n{:?}", p.befunde);
    for s in SECRETS {
        assert!(!text.contains(s), "explanation leaks {s:?}:\n{text}");
    }
    assert!(text.starts_with("VA-KUE-MRN refused (8112)"), "{text}");
}

proptest! {
    // ekpabg-pruefung/SR-003 and SC-002: for any combination of mismatches, the answer
    // validates clean and the explanation holds no value.
    #[test]
    fn any_record_yields_a_valid_answer_and_a_clean_explanation(
        name in 0usize..3, first in 0usize..3, plz in any::<bool>(), number in 0usize..3, alle in any::<bool>(),
        vae in any::<bool>(), nwe in any::<bool>(), end in 0i32..60, holders in any::<bool>(),
    ) {
        let mut va = va_kue_mrn();
        let surname = ["Mustermann", "Musterman", "Schmidt"][name];
        let vorname = [Some("Erika"), Some("E."), None][first];
        set_name(&mut va, vorname, surname);
        if plz {
            set_standort(&mut va, |s| s.postleitzahl = "10117".parse().unwrap());
        }
        let nums: [&[(&str, &str)]; 3] = [&[("30", "1234567")], &[("30", "1234567"), ("30", "5550000")], &[("30", "5550000")]];
        with_numbers(&mut va, nums[number], alle);
        let mut b = record();
        b.andere_vorabstimmung = vae;
        b.neue_willenserklaerung = nwe;
        b.vertragsende = Some(date("2027-01-01").checked_add(jiff::Span::new().days(end)).unwrap());
        if holders {
            b.alle_inhaber_noetig = true;
            b.inhaber.push(Kunde::person(Anrede::Herr, Some("Max".parse().unwrap()), "Beispiel".parse().unwrap()));
        }
        let p = run(&va, &b);
        let text = format!("{p}\n{:?}", p.befunde);
        for s in ["Erika", "Mustermann", "Schmidt", "1234567", "5550000", "10117", "Hauptstraße"] {
            prop_assert!(!text.contains(s));
        }
    }
}
