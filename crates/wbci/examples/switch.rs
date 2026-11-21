//! A provider switch end to end: the gaining provider (EKPauf) and the losing provider
//! (EKPabg) exchange XML, each following the switch with its own `Process`.
//!
//! Run with `cargo run -p wbci --example switch`.

use wbci::process::Message;
use wbci::{
    AkmTr, Anfrage, Anrede, Anschluss, Calendar, Control, Direction, Ekp, Hausnummer, Kunde, Meldung, Meldungscode,
    Nachname, Onkz, OnkzRufnummer, Ortsname, Payload, Portierungszeitfenster, Postleitzahl, Process, Role, Rufnummer,
    Rufnummernportierung, SignaturId, SignaturText, Standort, Strassenname, Teq, Timestamp, UebernahmeLeitung,
    Vorabstimmung, VorabstimmungsFall, VorabstimmungsId, Vorname, calendar, pruefung,
};

type Result<T = ()> = std::result::Result<T, Box<dyn std::error::Error>>;

fn at(berlin: &str) -> Result<Timestamp> {
    let dt: jiff::civil::DateTime = berlin.parse()?;
    Ok(dt.to_zoned_datetime(calendar::berlin())?.timestamp())
}

fn control(at: Timestamp, serial: &str) -> Result<Control> {
    Ok(Control::new(at, SignaturId { issuer: SignaturText::new("CN=Example CA")?, serial: SignaturText::new(serial)? }))
}

/// What travels over the wire: written by one side, read by the other.
fn wire(payload: &Payload) -> Result<Payload> {
    let xml = payload.to_soap()?;
    Ok(Payload::from_xml(xml.as_bytes())?)
}

fn show(who: &str, p: &Process) {
    println!("  {who}: {}", p.state());
    for d in p.deadlines() {
        println!("    · {d}\n      {}", d.explanation);
    }
}

fn main() -> Result {
    let ekp = Ekp { auf: "DEU.EKP1".parse()?, abg: "DEU.EKP2".parse()? };
    let cal = Calendar::wbci();

    // 1. The EKPauf builds the pre-agreement and checks it before sending.
    let va = Vorabstimmung {
        id: VorabstimmungsId::allocate(&ekp.auf, 13)?,
        kundenwunschtermin: "2027-01-15".parse()?,
        endkunde: Kunde::person(Anrede::Frau, Some(Vorname::new("Erika")?), Nachname::new("Mustermann")?),
        weitere_anschlussinhaber: vec![],
        projekt: None,
        fall: VorabstimmungsFall::KueMrn {
            standort: Standort {
                strassenname: Strassenname::new("Hauptstraße")?,
                hausnummer: Hausnummer::new("12")?,
                hausnummern_zusatz: None,
                postleitzahl: Postleitzahl::new("10115")?,
                ort: Ortsname::new("Berlin")?,
            },
            rufnummernportierung: Rufnummernportierung {
                anschluss: Anschluss::Einzel {
                    rufnummern: vec![OnkzRufnummer { onkz: Onkz::new("30")?, rufnummer: Rufnummer::new("1234567")? }],
                    alle_rufnummern: true,
                },
                zeitfenster: Portierungszeitfenster::Zf2,
            },
        },
        ekp: ekp.clone(),
    };
    let sent = at("2026-11-23T09:00")?;
    let anfrage = Anfrage::from(va.clone());
    for issue in anfrage.issues_at(sent, &cal) {
        println!("{issue}");
    }
    let request = Payload::AnfrageRequest { control: control(sent, "1")?, anfrage: anfrage.clone() };
    println!("── EKPauf → EKPabg: annehmenAnfrageRequest\n{}", request.to_xml()?);

    let mut auf = Process::new(Role::EkpAuf, va.clone(), Direction::Sent, sent)?;

    // 2. The EKPabg reads it, starts its process and answers with a TEQ.
    let Payload::AnfrageRequest { anfrage: Anfrage::Vorabstimmung(received), .. } = wire(&request)? else {
        return Err("expected a pre-agreement".into());
    };
    let mut abg = Process::new(Role::EkpAbg, received.clone(), Direction::Received, sent)?;
    let teq = Teq::for_anfrage(&received.clone().into(), Meldungscode::Ok);
    let acked = at("2026-11-23T09:00:02")?;
    abg.apply(Direction::Sent, acked, teq.clone())?;
    auf.apply(Direction::Received, acked, teq)?;
    println!("── after the TEQ");
    show("EKPauf", &auf);
    show("EKPabg", &abg);

    // 3. The EKPabg decides its answer from its own record: the contract ends a week later
    //    than asked, so the check agrees with 8002 (NAT) on the contract end.
    let answered = at("2026-11-24T10:00")?;
    let mut bestand = pruefung::Bestand::new(received.endkunde.clone(), "D123".parse()?);
    bestand.standort = received.standort().cloned();
    bestand.rufnummern = vec![OnkzRufnummer { onkz: Onkz::new("30")?, rufnummer: Rufnummer::new("1234567")? }];
    bestand.vertragsende = Some("2027-01-22".parse()?);
    let decision = pruefung::check(&received, &bestand, answered, &cal);
    println!("── the EKPabg's check\n{decision}");
    let Meldung::RuemVa(ruem) = decision.antwort.clone() else { return Err("expected an agreement".into()) };
    let meldung = decision.antwort;
    let issues = meldung.issues_at(Some(&received.clone().into()), answered, &cal);
    assert!(issues.is_empty(), "{issues:?}");
    let message = Payload::MeldungRequest { control: control(answered, "2")?, meldung: meldung.clone() };
    println!("── EKPabg → EKPauf: annehmenMeldungRequest\n{}", message.to_xml()?);
    abg.apply(Direction::Sent, answered, meldung)?;
    let Payload::MeldungRequest { meldung: got, .. } = wire(&message)? else {
        return Err("expected a message".into());
    };
    for code in got.codes() {
        println!("  code {code}");
    }
    auf.apply(Direction::Received, answered, got.clone())?;
    let teq = Teq::for_meldung(&got, Meldungscode::Ok);
    auf.apply(Direction::Sent, answered, teq.clone())?;
    abg.apply(Direction::Received, answered, teq)?;
    println!("── after the RUEM-VA");
    show("EKPauf", &auf);

    // 4. The EKPauf takes over the line.
    let line = UebernahmeLeitung::LineId("DEU.EKP2.1754639821".parse()?);
    let akm = Meldung::from(AkmTr::uebernahme(&ruem, vec![line], false));
    let announced = at("2026-11-25T08:00")?;
    auf.apply(Direction::Sent, announced, akm.clone())?;
    abg.apply(Direction::Received, announced, akm)?;
    println!("── after the AKM-TR");
    show("EKPabg", &abg);
    println!("  EKPauf may now: {:?}", auf.next_actions(Role::EkpAuf));

    // 5. A duplicate is refused with the code the protocol prescribes.
    if let Err(refusal) = abg.apply(Direction::Received, announced, Message::Anfrage(anfrage)) {
        println!("── a resent pre-agreement is refused: {refusal}");
    }
    println!("── history ({} events)", abg.history().len());
    Ok(())
}
