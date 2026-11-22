# telco-interop

**The German telecom carrier interfaces in one dependency: WBCI and S/PRI, and one provider
switch across both.**

[![crates.io](https://img.shields.io/crates/v/telco-interop.svg)](https://crates.io/crates/telco-interop)
[![docs.rs](https://docs.rs/telco-interop/badge.svg)](https://docs.rs/telco-interop)
![MSRV 1.88](https://img.shields.io/badge/rustc-1.88+-orange.svg)

A provider switch in Germany crosses two interfaces: the gaining and the losing provider agree
it over WBCI, then the gaining provider orders the line from the network owner over S/PRI with
the same Vorabstimmungs-ID. This crate re-exports both under one name and one version and
adds `Wechsel`, which shows one switch across both.

```toml
telco-interop = "0.1"   # wbci + spri; or one of them with default-features = false
```

```rust
use telco_interop::{VorabstimmungsId, spri, wbci};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Agreed over WBCI …
    let id: wbci::VorabstimmungsId = "DEU.EKP1.V000000013".parse()?;
    // … and the very same type in an S/PRI provider-switch order: no conversion.
    let in_spri: spri::VorabstimmungsId = id.clone();
    let shared: VorabstimmungsId = in_spri;
    assert_eq!(shared.as_str(), "DEU.EKP1.V000000013");
    Ok(())
}
```

| | Interface | Between | Feature |
|---|---|---|---|
| [`wbci`](https://crates.io/crates/wbci) | WBCI 2.0 | provider ↔ provider: the § 59 TKG switch, number porting, the losing provider's check | `wbci` |
| [`spri`](https://crates.io/crates/spri) | S/PRI 4.x | service provider ↔ network owner: open-access orders, faults, the provider switch | `spri` |
| `Wechsel` | both | one switch: one timeline, both interfaces' deadlines, next actions per party, six agreement checks | both |

`Wechsel` holds the WBCI process and every S/PRI order with its Vorabstimmungs-ID and checks
that the interfaces agree: agreed before ordered, the line the AKM-TR takes over, the dates,
postponement in WBCI first, a WBCI cancellation reaching S/PRI, completion within a working
day of the switch date. Its timeline reads like this:

```text
Mo 23.11.2026 09:00 WBCI EKPauf → VA-KUE-MRN
Di 24.11.2026 10:00 WBCI EKPabg → RUEM-VA 8001
…
```

**[Guide](https://hupe1980.github.io/telco-interop/docs/across-both/one-switch/)** ·
**[All guides](https://hupe1980.github.io/telco-interop/)** · **[API](https://docs.rs/telco-interop)**

## Sandbox

`telco_interop::sim` plays 13 named scenarios from both sides against simulated peers
(`wbci::sim`, `spri::sim`), to test your side without a partner: WBCI agreed, refused, Storno
and TVS-VA; S/PRI NEU, TAM to 1198, Storno, Terminverschiebung, PV, its refusal and timeout;
the switch across both. Every message validates, is valid against both official schemas and
meets its deadline.

```rust
use telco_interop::sim;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let ablauf = sim::szenario("spri-pv").ok_or("no such scenario")?.play(sim::START);
    for nachricht in &ablauf.nachrichten {
        println!("{nachricht}"); // Mo 01.03.2027 09:00 S/PRI EKPauf → Netzbetreiber PV, …
    }
    Ok(())
}
```

**[Guide](https://hupe1980.github.io/telco-interop/docs/across-both/sandbox/)**

## Command line

The `cli` feature (not on by default) adds the `telco-interop` command:

```sh
brew install hupe1980/tap/telco-interop      # or: cargo install telco-interop --features cli
telco-interop read abbm.xml                  # what a file is, or the TEQ code to refuse it with
telco-interop check va.xml auftrag.xml       # WBCI and S/PRI validation issues, with rule, source and code
telco-interop code 8103                      # a Meldungscode in WBCI and S/PRI
telco-interop timeline --as ekpauf switch/   # one switch replayed: timeline, findings, deadlines
telco-interop simulate wechsel switch/       # a sandbox scenario's files, both sides (--list for all)
```

It prints no personal data and exits 1 on an error, a finding or an overdue obligation.
**[Guide](https://hupe1980.github.io/telco-interop/docs/start/command-line/)** ·
**[Try `read`, `check` and `code` in the browser](https://hupe1980.github.io/telco-interop/playground/)**

## The command line's output as a library

The `report` feature (not on by default, sans-I/O) gives what `read`, `check` and `code` print
as text lines from bytes, for another front end — the site's playground uses it:

```rust
use telco_interop::report;

let out = report::code("8103", true, true); // WBCI and S/PRI
assert!(out.ok);
assert_eq!(out.lines[0], "WBCI 8103 Rufnummer nicht geschaltet (RNG)");
assert!(!report::read(b"<!DOCTYPE x><x/>").ok); // unreadable, with the TEQ code to answer with
```

The lines are value-free (kinds, ids, dates, codes) and control characters arrive as U+FFFD.

Need only one interface? Depend on `wbci` or `spri` directly; they are the same code.

Unofficial; not affiliated with or endorsed by the AK S&P, VATM, BUGLAS or Deutsche Telekom.

## License

Licensed under either of [Apache License, Version 2.0](LICENSE-APACHE) or
[MIT license](LICENSE-MIT) at your option.
