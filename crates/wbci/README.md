# wbci

**WBCI 2.0, the German provider switch (§ 59 TKG), for Rust.**

[![crates.io](https://img.shields.io/crates/v/wbci.svg)](https://crates.io/crates/wbci)
[![docs.rs](https://docs.rs/wbci/badge.svg)](https://docs.rs/wbci)
[![CI](https://github.com/hupe1980/telco-interop/actions/workflows/ci.yml/badge.svg)](https://github.com/hupe1980/telco-interop/actions/workflows/ci.yml)
![MSRV 1.88](https://img.shields.io/badge/rustc-1.88+-orange.svg)

Before a fixed line changes provider in Germany, the gaining provider (EKPauf) and the losing
provider (EKPabg) agree the switch over WBCI, the AK S&P's provider-to-provider interface:
a pre-agreement (*Vorabstimmung*), a binding date or a coded refusal, the line takeover,
cancellations and postponements, all under working-day deadlines. This crate gives you that
protocol as typed values, XML and a state machine, without I/O.

```rust
use wbci::*;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Read what a peer sends; a broken payload names the receipt code to answer with.
    if let Err(e) = Payload::from_xml(b"<!DOCTYPE x []><x/>") {
        println!("{e} → TEQ {}", e.teq_code().as_str()); // … → TEQ 0995
    }

    // A pre-agreement acknowledged before Christmas: when must the losing provider answer?
    let teq: Timestamp = "2026-12-23T15:00:00Z".parse()?;
    let due = deadline::va_answer(teq, &Calendar::wbci());
    assert_eq!(due.party, Role::EkpAbg);
    println!("{due}");               // EKPabg: by Di 29.12.2026 23:59 — … (Anl. 1 V5.4.1 §2.5, §2.6.1)
    println!("{}", due.explanation); // counted Do 24.12., Mo 28.12., Di 29.12.; skipped 25.–27.12. and why

    let code: Meldungscode = "8103".parse()?;
    println!("{code}");              // 8103 Rufnummer nicht geschaltet (RNG)
    Ok(())
}
```

| | |
|---|---|
| **Requests** | `VA-KUE-MRN`, `VA-KUE-ORN`, `VA-RRNP`, `STR-AUF`, `STR-AEN` (either side), `TVS-VA` |
| **Messages** | `RUEM-VA`, `ABBM`, `ERLM`, `AKM-TR`, `ABBM-TR`, `TEQ`; answers built from what they answer |
| **XML** | the four payloads, bare or in SOAP 1.1; byte-stable; 373 payloads validated against the official XSD V2.00 in CI; DTDs refused, input bounded, fuzzed |
| **Validation** | 28 cited rules, each issue with path, rule, source and the peer's answer code |
| **Deadlines** | every rule of Anlage 1 §2.5–§2.11 in German time, with counted and skipped days |
| **Process** | state, next steps per side, open and overdue deadlines, refusals with codes, history |
| **Losing provider's check** | `pruefung::check` decides the RUEM-VA or ABBM from your record: names by sound (AK S&P word rules, Kölner Phonetik), address, numbers, holders, explained field by field |
| **Codes** | all 36 Meldungscodes with text, message kind and the cases they answer |

**[Guides](https://hupe1980.github.io/telco-interop/docs/wbci/)** ·
**[Getting started](https://hupe1980.github.io/telco-interop/docs/start/getting-started/)** ·
**[API](https://docs.rs/wbci)** · example: `cargo run -p wbci --example switch`

Implements the AK S&P's WBCI Schnittstellenbeschreibung (interface 2.0), XSD V2.00, Anlage 1
„Spezifikation Vorabstimmung" V5.4.1 and the Arbeitshandbuch WBCI 2.10; interpretations are
[documented](https://hupe1980.github.io/telco-interop/docs/reference/scope/). Personal data is
redacted in `Debug` and never appears in errors. Speak S/PRI too? See
[`telco-interop`](https://crates.io/crates/telco-interop).

Unofficial; not affiliated with or endorsed by the AK S&P, VATM or BUGLAS.

## License

Licensed under either of [Apache License, Version 2.0](LICENSE-APACHE) or
[MIT license](LICENSE-MIT) at your option.
