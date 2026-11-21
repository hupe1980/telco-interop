//! Each rule, broken alone, yields exactly one issue with its path, rule, source and code.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::indexing_slicing)]

mod common;

use common::*;
use wbci::*;

fn sent() -> Timestamp {
    let dt: jiff::civil::DateTime = "2026-11-23T09:00".parse().unwrap();
    dt.to_zoned_datetime(calendar::berlin()).unwrap().timestamp()
}

fn one(issues: &[Issue]) -> &Issue {
    assert_eq!(issues.len(), 1, "{issues:#?}");
    &issues[0]
}

#[test]
fn every_sample_is_clean() {
    let cal = Calendar::wbci();
    for a in [
        va_kue_mrn().into(),
        va_kue_orn().into(),
        str_auf_auf().into(),
        str_aen_auf().into(),
        str_aen_abg().into(),
        tvs().into(),
    ] {
        let a: Anfrage = a;
        assert!(a.issues_at(sent(), &cal).is_empty(), "{:#?}", a.issues_at(sent(), &cal));
    }
    let va: Anfrage = va_kue_mrn().into();
    assert!(Meldung::from(ruem_va()).issues(Some(&va)).is_empty(), "{:#?}", Meldung::from(ruem_va()).issues(Some(&va)));
    assert_eq!(Meldung::from(akm_tr()).issues(Some(&va)), vec![]);
    assert_eq!(Meldung::from(abbm_tr()).issues(Some(&va)), vec![]);
    assert_eq!(Meldung::from(erlm()).issues(Some(&tvs().into())), vec![]);
}

#[test]
fn lead_time_of_a_pre_agreement() {
    let mut va = va_kue_mrn();
    va.kundenwunschtermin = date("2026-12-07"); // 10 AT, a Monday
    let issue = one(&Anfrage::from(va).issues_at(sent(), &Calendar::wbci())).clone();
    assert_eq!(issue.rule, "va.lead");
    assert_eq!(issue.severity, Severity::Warning);
    assert_eq!(issue.path, "Anfrage/Kundenwunschtermin");
    assert_eq!(issue.source, "Anl. 1 V5.4.1 §2.5");
    assert!(issue.message.contains("2026-12-21"), "{issue}");
}

#[test]
fn requested_date_on_a_holiday() {
    let mut va = va_kue_mrn();
    va.kundenwunschtermin = date("2027-03-29"); // Ostermontag
    let issue = one(&Anfrage::from(va).issues_at(sent(), &Calendar::wbci())).clone();
    assert_eq!(issue.rule, "termin.working-day");
    assert!(issue.message.contains("Ostermontag"));
}

#[test]
fn id_from_another_carrier() {
    let mut va = va_kue_mrn();
    va.id = VorabstimmungsId::allocate(&cc("DEU.EKP2"), 1).unwrap();
    assert_eq!(one(&Anfrage::from(va).issues()).rule, "id.issuer");
}

#[test]
fn same_carrier_on_both_sides() {
    let mut va = va_kue_mrn();
    va.ekp.abg = va.ekp.auf.clone();
    assert!(Anfrage::from(va).issues().iter().any(|i| i.rule == "ekp.distinct"));
}

#[test]
fn eleven_numbers() {
    let mut va = va_kue_mrn();
    va.fall = VorabstimmungsFall::KueMrn {
        standort: standort(),
        rufnummernportierung: Rufnummernportierung {
            anschluss: Anschluss::Einzel {
                rufnummern: (0..11).map(|i| nummer("30", &format!("12345{i:02}"))).collect(),
                alle_rufnummern: false,
            },
            zeitfenster: Portierungszeitfenster::Zf2,
        },
    };
    let issue = one(&Anfrage::from(va).issues()).clone();
    assert_eq!(issue.rule, "schema.count");
    assert_eq!(issue.code, Some(Meldungscode::DatenstrukturUngueltig));
    assert_eq!(issue.path, "Anfrage/RufnummernPortierung/einzelanschluss/rufnummernliste/zuPortierendeOnkzRnr");
}

#[test]
fn reversed_number_block() {
    let mut va = va_rrnp();
    va.fall = VorabstimmungsFall::Rrnp {
        rufnummernportierung: Rufnummernportierung {
            anschluss: Anschluss::Anlage {
                kopf: Anlagenkopf {
                    onkz: "228".parse().unwrap(),
                    durchwahl: "4711".parse().unwrap(),
                    abfragestelle: "0".parse().unwrap(),
                },
                bloecke: vec![Rufnummernblock { von: "99".parse().unwrap(), bis: "00".parse().unwrap() }],
            },
            zeitfenster: Portierungszeitfenster::Zf2,
        },
        pki_auf: "D001".parse().unwrap(),
    };
    let issue = one(&Anfrage::from(va).issues()).clone();
    assert_eq!(issue.rule, "block.range");
    assert_eq!(issue.code, Some(Meldungscode::Rng));
}

#[test]
fn zf3_needs_an_agreement() {
    let mut va = va_kue_mrn();
    if let VorabstimmungsFall::KueMrn { rufnummernportierung, .. } = &mut va.fall {
        rufnummernportierung.zeitfenster = Portierungszeitfenster::Zf3;
    }
    assert_eq!(one(&Anfrage::from(va).issues()).rule, "zeitfenster.zf3");
}

#[test]
fn code_of_another_message_kind() {
    let mut abbm = abbm();
    abbm.positionen[0].position = Position::new(Meldungscode::Zwa);
    abbm.positionen[0].rufnummern.clear();
    let issue = one(&Meldung::from(abbm).issues(None)).clone();
    assert_eq!(issue.rule, "code.applicability");
    assert_eq!(issue.path, "Meldung/Position[1]/Meldungscode");
    assert!(issue.message.contains("8001 may not answer VA-KUE-MRN in a ABBM"), "{issue}");
}

#[test]
fn code_not_for_this_business_case() {
    let mut abbm = abbm();
    abbm.kopf.geschaeftsfall = Geschaeftsfall::VaRrnp;
    abbm.positionen = vec![Position::new(Meldungscode::Nwe).into()];
    assert_eq!(one(&Meldung::from(abbm).issues(None)).rule, "code.applicability");
}

#[test]
fn unknown_code() {
    let mut abbm = abbm();
    abbm.positionen = vec![Position::new(Meldungscode::parse("9999")).into()];
    assert_eq!(one(&Meldung::from(abbm).issues(None)).rule, "code.unknown");
}

#[test]
fn numbers_only_with_rng() {
    let mut abbm = abbm();
    abbm.positionen[0].position = Position::new(Meldungscode::Kni);
    assert_eq!(one(&Meldung::from(abbm).issues(None)).rule, "abbm.rufnummern");
}

#[test]
fn son_needs_a_reason() {
    let mut abbm = abbm();
    abbm.positionen = vec![Position::new(Meldungscode::Sonstiges).into()];
    abbm.begruendung = None;
    assert_eq!(one(&Meldung::from(abbm).issues(None)).rule, "abbm.son-begruendung");
}

#[test]
fn ruem_va_needs_one_agreement_code() {
    let mut ruem = ruem_va();
    ruem.positionen.remove(0);
    assert_eq!(one(&Meldung::from(ruem).issues(None)).rule, "ruem-va.zustimmung");
}

#[test]
fn ada_code_without_its_address_part() {
    let mut ruem = ruem_va();
    ruem.positionen[1].adresse_abweichend = Some(AdresseAbweichend::Ort("Bonn".parse().unwrap()));
    assert_eq!(one(&Meldung::from(ruem).issues(None)).rule, "ruem-va.ada");
}

#[test]
fn zwa_with_another_date() {
    let mut ruem = ruem_va();
    ruem.wechseltermin = date("2027-01-22");
    let va: Anfrage = va_kue_mrn().into();
    assert_eq!(one(&Meldung::from(ruem).issues(Some(&va))).rule, "ruem-va.zwa");
}

#[test]
fn switch_date_too_early_when_answering() {
    let va = va_kue_mrn();
    let mut late_request = va.clone();
    late_request.kundenwunschtermin = date("2026-12-01");
    let ruem = RuemVa::zustimmung(&late_request, date("2026-12-01"));
    let issue = one(&Meldung::from(ruem).issues_at(Some(&late_request.into()), sent(), &Calendar::wbci())).clone();
    assert_eq!(issue.rule, "ruem-va.date");
    assert!(issue.message.contains("2026-12-14"));
}

#[test]
fn ruem_va_from_the_wrong_side() {
    let mut ruem = ruem_va();
    ruem.kopf.absender = cc("DEU.EKP1");
    assert_eq!(one(&Meldung::from(ruem).issues(None)).rule, "absender.role");
}

#[test]
fn answer_to_another_pre_agreement() {
    let other: Anfrage = va_kue_orn().into();
    let issues = Meldung::from(ruem_va()).issues(Some(&other));
    assert!(
        issues.iter().any(|i| i.rule == "meldung.answers" && i.code == Some(Meldungscode::VorabstimmungsIdUnbekannt))
    );
}

#[test]
fn refused_postponement_returns_the_date() {
    let mut abbm = Abbm::ablehnung(&tvs().into(), Meldungscode::TerminverschiebungAbgelehnt);
    assert_eq!(one(&Meldung::from(abbm.clone()).issues(Some(&tvs().into()))).rule, "abbm.tvs-termin");
    abbm.wechseltermin = Some(date("2027-01-15"));
    assert_eq!(Meldung::from(abbm).issues(Some(&tvs().into())), vec![]);
}

#[test]
fn accepted_postponement_confirms_the_requested_date() {
    let mut erlm = Erlm::terminverschiebung_ok(&tvs());
    erlm.wechseltermin = Some(date("2027-02-05"));
    assert_eq!(one(&Meldung::from(erlm).issues(Some(&tvs().into()))).rule, "erlm.tvs-termin");
}

#[test]
fn answer_to_a_cancellation_refers_to_it() {
    let mut erlm = Erlm::storno_ok(&str_auf_auf());
    erlm.storno_id = None;
    assert_eq!(one(&Meldung::from(erlm).issues(None)).rule, "meldung.ref");
}

#[test]
fn akm_tr_lines_only_with_takeover() {
    let mut akm = akm_tr();
    akm.ressourcenuebernahme = false;
    assert_eq!(one(&Meldung::from(akm).issues(None)).rule, "akm-tr.leitung");
}

#[test]
fn issues_display_with_rule_source_and_code() {
    let mut abbm = abbm();
    abbm.positionen = vec![Position::new(Meldungscode::parse("9999")).into()];
    let text = Meldung::from(abbm).issues(None)[0].to_string();
    assert_eq!(
        text,
        "error [code.unknown] Meldung/Position[1]/Meldungscode: 9999 is not in the published code list (AH4WBCI 2.10 §2, §5.3)"
    );
}

#[test]
fn receipts_carry_receipt_codes_only() {
    assert_eq!(teq_ok().issues(), vec![]);
    let mut teq = teq_ok();
    teq.positionen = vec![Position::new(Meldungscode::Zwa)];
    assert_eq!(one(&teq.issues()).rule, "code.applicability");
    teq.positionen = vec![Position::new(Meldungscode::Ok), Position::new(Meldungscode::DatenstrukturUngueltig)];
    assert_eq!(one(&teq.issues()).rule, "teq.mixed");
    teq.positionen.clear();
    assert_eq!(one(&teq.issues()).rule, "schema.count");
}
