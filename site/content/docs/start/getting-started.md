+++
title = "Getting started"
description = "Install the crates, build a WBCI pre-agreement (Vorabstimmung), check it, write the XML, read the answer and follow the deadlines."
weight = 2

[extra]
seo_title = "Getting started with WBCI and S/PRI in Rust · telco-interop"
+++

## Install

Pick the interface you speak, or both:

```sh
cargo add wbci             # provider ↔ provider (the § 59 TKG switch)
cargo add spri             # service provider ↔ network owner (open access)
cargo add telco-interop    # both, re-exported as telco_interop::{wbci, spri}, plus Wechsel
```

Rust 1.88 or newer. Three external dependencies: `quick-xml`, `jiff` (dates and times, with
the German time zone compiled in) and `thiserror`. The rest of this page uses `wbci`; the
[S/PRI guide](@/docs/spri/orders-and-messages.md) starts the same way.

## Build, check, write

A pre-agreement names the two providers, the customer, the requested date and the case; here
a switch with number porting (`VA-KUE-MRN`):

{% <rust> %}
```rust
use wbci::*;

# fn main() -> Result<(), Box<dyn std::error::Error>> {
let ekp = Ekp { auf: "DEU.EKP1".parse()?, abg: "DEU.EKP2".parse()? };
let va = Vorabstimmung {
    // Your sequence, your carrier code: DEU.EKP1.V000000042.
    id: VorabstimmungsId::allocate(&ekp.auf, 42)?,
    kundenwunschtermin: "2027-01-15".parse()?,
    endkunde: Kunde::person(Anrede::Herr, Some(Vorname::new("Max")?), Nachname::new("Mustermann")?),
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
    ekp,
};

// Would the peer refuse it? Lead times count from when it is sent.
let sent: Timestamp = "2026-11-23T08:00:00Z".parse()?;
let anfrage = Anfrage::from(va);
for issue in anfrage.issues_at(sent, &Calendar::wbci()) {
    println!("{issue}");
}

let signatur = SignaturId { issuer: SignaturText::new("CN=Example CA")?, serial: SignaturText::new("1")? };
let payload = Payload::AnfrageRequest { control: Control::new(sent, signatur), anfrage };
let body = payload.to_xml()?;      // the annehmenAnfrageRequest element
let soap = payload.to_soap()?;     // or a SOAP 1.1 envelope with an empty header for WS-Security
assert!(body.contains("xsi:type=\"ar:KuendigungMitRNPGeschaeftsfallType\""));
assert!(soap.contains("<soapenv:Body>"));
# Ok(())
# }
```
{% </rust> %}

Values check themselves when built: `Postleitzahl::new("1011")` fails with
`Postleitzahl: length must be 5, is 4`, and `Vorname::new("An\u{7}na")` names the character
and its position, never the value. Mandatory elements are plain fields, so leaving one out is
a compile error.

## Read what comes back

{% <rust> %}
```rust
use wbci::*;

# fn main() -> Result<(), Box<dyn std::error::Error>> {
# let xml = std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/abbm.xml"))?;
match Payload::from_xml(xml.as_bytes()) {
    Ok(Payload::MeldungRequest { meldung, .. }) => {
        for code in meldung.codes() {
            println!("{code}");   // 8103 Rufnummer nicht geschaltet (RNG)
        }
    }
    Ok(other) => println!("{:?}", other.control()),
    // Answer the call with this code in the TEQ: 0993 or 0995.
    Err(e) => println!("{} → TEQ {}", e, e.teq_code().as_str()),
}
# Ok(())
# }
```
{% </rust> %}

## Follow the switch

Feed every request, message and receipt into a `Process`; it tells you what is due:

{% <rust> %}
```rust
use wbci::*;

# fn main() -> Result<(), Box<dyn std::error::Error>> {
# let xml = std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/va-kue-mrn.xml"))?;
# let Payload::AnfrageRequest { anfrage: Anfrage::Vorabstimmung(va), .. } = Payload::from_xml(xml.as_bytes())? else { unreachable!() };
let sent: Timestamp = "2026-11-23T08:00:00Z".parse()?;
let mut process = Process::new(Role::EkpAuf, va.clone(), Direction::Sent, sent)?;
process.apply(Direction::Received, sent, Teq::for_anfrage(&va.into(), Meldungscode::Ok))?;
for deadline in process.deadlines() {
    println!("{deadline}");   // EKPabg: by Do 26.11.2026 23:59 — … (Anl. 1 V5.4.1 §2.5, §2.6.1)
}
# assert!(process.deadlines()[0].to_string().starts_with("EKPabg: by Do 26.11.2026 23:59"));
# Ok(())
# }
```
{% </rust> %}

The repository has the whole switch as a runnable program: `cargo run -p wbci --example switch`.

## Next

- [Requests and messages](@/docs/wbci/requests-and-messages.md): every kind, and answers built from requests.
- [Deadlines](@/docs/wbci/deadlines.md) and [the process](@/docs/wbci/process.md): what is due, and what may happen next.
- [Answering as the losing provider](@/docs/wbci/pruefung.md): the check, decided from your record.
- [Glossary](@/docs/start/glossary.md): EKPauf, RUEM-VA, AT and every other term.
