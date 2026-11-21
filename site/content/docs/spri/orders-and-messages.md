+++
title = "S/PRI orders and messages"
description = "Every S/PRI 4.x order and message of German open access as Rust types: NEU, KUE-AG, LAE, AEN-LMAE, EST, PV, GET/SET; QEB, ABM, ABBM, ERLM, ENTM and the rest."
weight = 1

[extra]
nav = "Orders and messages"
seo_title = "S/PRI 4.x orders and messages in Rust · telco-interop"
+++

S/PRI is how a service provider (*Auftraggeber*) orders lines from a fibre network owner
(*Leistungserbringer*) and receives its answers. The `spri` crate covers all of S/PRI 4.x
(schema 4.00.03).

## Orders

An `Auftrag` carries one `Geschaeftsfall` and an `Aenderungskennzeichen`: `Standard` for a new
order, `Storno` to cancel it, `Terminverschiebung` to move its date (same order number).

| Business case | `Geschaeftsfall` | For |
|---|---|---|
| `NEU` | `Neu(Bereitstellung)` | a new line |
| `KUE-AG` | `KueAg(Kuendigung)` | the service provider cancels a line |
| `LAE` | `Lae(Leistungsaenderung)` | change the service (product) |
| `AEN-LMAE` | `AenLmae(LeistungsmerkmalAenderung)` | change features |
| `EST` | `Est(Entstoerung)` | a fault report |
| `PV` | `Pv(Providerwechsel)` | take over a line after a WBCI pre-agreement |
| `GET`, `SET` | `Get(Diagnose)`, `Set(Diagnose)` | read diagnostic values, trigger diagnostic operations |

Orders carry the FTTx products, appointments, contacts and an attachment the schema defines.

## Messages

A `Meldung` carries the business case it answers (`Auftragstyp`) and one `Meldungsinhalt`:

| From the network owner | From the service provider | To the losing provider (PV) |
|---|---|---|
| `QEB` `ABM` `ABBM` `ERLM` `ENTM` `VZM` `TAM` `MTAM` `KDLE` `ZWM-LE` `TBK-LE` | `ZWM-AG` `TBK-AG` `ERLM-K` | `AKM-PV` `ABM-PV` `ERLM-PV` `ENTM-PV` `ABBM-PV`, answered with `RUEM-PV` |

Every call is acknowledged with a technical receipt, `Teq::new(code, …)`. The
[glossary](@/docs/start/glossary.md#s-pri-messages) explains each abbreviation.

## Write a provider-switch order

The Vorabstimmungs-ID is the same type as in the `wbci` crate, so the id of an agreed
pre-agreement goes into the `PV` order without conversion:

{% <rust> %}
```rust
use spri::*;

# fn main() -> Result<(), Box<dyn std::error::Error>> {
let kontakt = Ansprechpartner {
    person: Person { anrede: Anrede::Frau, vorname: None, nachname: Nachname::new("Auftragsmanagement")? },
    telefonnummer: Telefonnummer::new("030 1234567")?,
    mobilfunknummer: None,
    faxnummer: None,
    emailadresse: None,
};
let pv = Providerwechsel {
    line_id: "DEU.EKP2.1754639821".parse()?,
    vorabstimmungs_id: "DEU.EKP1.V000000013".parse()?, // from the WBCI RUEM-VA
    anlage: None,
    ansprechpartner: AnsprechpartnerAuftrag { auftragsmanagement: kontakt, weitere: vec![] },
    wunschtermin: "2027-01-15".parse::<Date>()?.into(),
    produkt: Produkt { bezeichner: Produktbezeichner::new("FTTH 1000")? },
    fttx: FttxProviderwechsel {
        standort_a: StandortA {
            inhaber: Inhaber::Person(Person {
                anrede: Anrede::Frau,
                vorname: Some(Vorname::new("Erika")?),
                nachname: Nachname::new("Mustermann")?,
            }),
            telefonnummer: None,
            mobilfunknummer: None,
            strasse: Strasse {
                strassenname: Strassenname::new("Hauptstraße")?,
                hausnummer: Hausnummer::new("12")?,
                hausnummern_zusatz: None,
            },
            gebaeudeteil: None,
            land: None,
            postleitzahl: Postleitzahl::new("10115")?,
            ort: Ort { ortsname: Ortsname::new("Berlin")?, ortsteil: None },
            lage_tae_ont: None,
        },
        montageleistung: None,
    },
    positionen: vec![],
};
let auftrag = Auftrag {
    externe_auftragsnummer: ExterneAuftragsnummer::new("A-2026-0001")?,
    auftraggeber: Auftraggeber {
        auftraggebernummer: Auftraggebernummer::new("4711")?,
        leistungsnummer: Leistungsnummer::new("RV-1")?,
    },
    besteller: None,
    geschaeftsfall: Geschaeftsfall::Pv(pv),
    geschaeftsfall_art: GeschaeftsfallArt::Endkundenanbieterwechsel,
    aenderungskennzeichen: Aenderungskennzeichen::Standard,
    termin_neu: None,
    kenner: None,
};

let sent: Timestamp = "2026-11-23T08:00:00Z".parse()?;
let signatur = SignaturId { issuer: SignaturText::new("CN=Example CA")?, serial: SignaturText::new("4711")? };
let payload = Payload::AuftragRequest { control: Control::new(sent, signatur), auftrag };
let xml = payload.to_xml()?;
assert!(xml.contains(r#"<geschaeftsfall xsi:type="order:ProviderwechselType">"#));
assert!(xml.contains("<vorabstimmungId>DEU.EKP1.V000000013</vorabstimmungId>"));

// The network owner reads it back as the same value.
assert_eq!(Payload::from_xml(xml.as_bytes())?, payload);
# Ok(())
# }
```
{% </rust> %}

`Payload::to_soap()` wraps the same payload in a SOAP 1.1 envelope with an empty header for
your WS-Security signer.

## Read what the network owner sends

{% <rust> %}
```rust
use spri::*;

# fn main() -> Result<(), Box<dyn std::error::Error>> {
# let xml = std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/meldung-abm.xml"))?;
match Payload::from_xml(xml.as_bytes()) {
    Ok(Payload::MeldungRequest { meldung, .. }) => {
        println!("{} for {}", meldung.typ(), meldung.auftragstyp.geschaeftsfall);
        for code in meldung.codes() {
            println!("  {code}");   // e.g. 0004 with its text from the Arbeitshandbuch
        }
    }
    Ok(_) => {}
    // Answer the call with this receipt code: 0993 (version) or 0995 (structure).
    Err(e) => println!("{e} → TEQ {}", e.teq_code().as_str()),
}
# Ok(())
# }
```
{% </rust> %}

## One foundation with WBCI

`spri::VorabstimmungsId`, `LineId` and `CarrierCode` are the same types as in `wbci` (both
come from `carrier-core`), as are the calendar, the working-day arithmetic, the envelope and
the hardened XML reader. [One switch](@/docs/across-both/one-switch.md) follows both
interfaces together.

## Attachments

Orders and some messages carry one attachment (`Anlage`: file name, type, base64 content, held
as bytes, at most 1 MiB by default — `xml::Limits`). The file name comes from another company:

{% <rust> %}
```rust
# use spri::*;
# fn main() -> Result<(), Box<dyn std::error::Error>> {
# let xml = std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/auftrag-neu.xml"))?;
# let anlage_xml = xml.replacen("<dateiname>", "<dateiname>../../etc/", 1);
# let Payload::AuftragRequest { auftrag, .. } = Payload::from_xml(anlage_xml.as_bytes())? else { unreachable!() };
# let Geschaeftsfall::Neu(neu) = auftrag.geschaeftsfall else { unreachable!() };
# let anlage = neu.anlage.expect("the NEU fixture has an attachment");
// Never join `dateiname` to a folder as it is: use the safe name.
let name = anlage.safe_file_name();
assert!(!name.contains('/') && !name.contains('\\') && !name.starts_with('.'));
# Ok(())
# }
```
{% </rust> %}

`Debug` prints an attachment's size, never its content or name; names, phone numbers, e-mail
addresses, street and house number are redacted too.

## Next

- [The order lifecycle](@/docs/spri/lifecycle.md): what may follow what, and what is due.
- [S/PRI message codes](@/docs/spri/codes.md): all 215, and what an ABBM means for the order.
- [Schema conformance](@/docs/reference/conformance.md): the 776-payload corpus.
