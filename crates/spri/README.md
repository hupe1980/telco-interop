# spri

**S/PRI 4.x, the German open-access fibre interface, for Rust.**

[![crates.io](https://img.shields.io/crates/v/spri.svg)](https://crates.io/crates/spri)
[![docs.rs](https://docs.rs/spri/badge.svg)](https://docs.rs/spri)
[![CI](https://github.com/hupe1980/telco-interop/actions/workflows/ci.yml/badge.svg)](https://github.com/hupe1980/telco-interop/actions/workflows/ci.yml)
![MSRV 1.88](https://img.shields.io/badge/rustc-1.88+-orange.svg)

On open-access networks a service provider (*Auftraggeber*) orders, changes, cancels and
repairs lines at the network owner (*Leistungserbringer*) over S/PRI, the AK S&P's interface;
a provider switch agreed over WBCI arrives here as a `PV` order. This crate gives you every
order and message as typed values, XML proven against the official schema and the lifecycle
of each order, without I/O.

```rust
use spri::*;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Read what the network owner sends; a broken payload names the receipt code.
    if let Err(e) = Payload::from_xml(b"<!DOCTYPE x []><x/>") {
        assert_eq!(e.teq_code(), Meldungscode::DATENSTRUKTUR_UNZULAESSIG); // TEQ 0995
    }

    // Every code of the Arbeitshandbuch, and what an ABBM with it means for the order.
    let code: Meldungscode = "1198".parse()?;
    println!("{code}"); // 1198 Es wurde kein neuer Ausführungstermin übermittelt.
    assert_eq!(code.abbm_outcome(GeschaeftsfallCode::Lae, AbbmPhase::AfterAbm), AbbmOutcome::Closed);

    // A TAM unanswered for 10 Werktage (Mon–Sat) brings the MTAM, explained.
    let tam: Timestamp = "2026-11-23T08:00:00Z".parse()?;
    println!("{}", deadline::mtam_due(tam, &Calendar::werktage()));
    Ok(())
}
```

| | |
|---|---|
| **Orders** | `NEU`, `KUE-AG`, `LAE`, `AEN-LMAE`, `EST`, `PV`, `GET`/`SET`, with Storno and Terminverschiebung; FTTx products, appointments, contacts, attachments |
| **Messages** | all 20: `QEB` `ABM` `ABBM` `ERLM` `ENTM` `VZM` `TAM` `MTAM` `KDLE` `ZWM-LE/-AG` `TBK-LE/-AG` `ERLM-K` and to the losing provider `AKM-PV` `RUEM-PV` `ABM-PV` `ERLM-PV` `ENTM-PV` `ABBM-PV`; the `TEQ` |
| **XML** | the four payloads, bare or in SOAP 1.1; byte-stable; 776 payloads validated against XSD 4.00.03 in CI; DTDs refused, input and attachments bounded, fuzzed |
| **Lifecycle** | `Process` per order and role, after the EPKs of Anlage 7 V.13: what may follow what, refusals naming the ABBM to answer with, next actions, explained deadlines, history; `Abbm::outcome` |
| **Codes** | the 215 codes of the Arbeitshandbuch S/PRI 4.x V2.9.5 with text, message types and business cases |
| **With WBCI** | `VorabstimmungsId`, `LineId`, `CarrierCode` are the same types as in [`wbci`](https://crates.io/crates/wbci) |
| **Safety** | personal and contact data redacted in `Debug`; `Anlage::safe_file_name()` against path traversal |

**[Guides](https://hupe1980.github.io/telco-interop/docs/spri/)** ·
**[API](https://docs.rs/spri)** (with a complete `PV` order)

Implements the AK S&P's Providervereinbarung S/PRI V4.4, schema 4.00.03 (all 4.x versions),
Anlage 7 „EPKs der Geschäftsfälle" V.13 and the Arbeitshandbuch S/PRI 4.x V2.9.5;
interpretations are [documented](https://hupe1980.github.io/telco-interop/docs/reference/scope/).

Unofficial; not affiliated with or endorsed by the AK S&P, VATM, BUGLAS or Deutsche Telekom.

## License

Licensed under either of [Apache License, Version 2.0](LICENSE-APACHE) or
[MIT license](LICENSE-MIT) at your option.
