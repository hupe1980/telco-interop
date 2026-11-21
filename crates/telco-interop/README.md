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

## Command line

The `cli` feature (not on by default) adds the `telco-interop` command:

```sh
brew install hupe1980/tap/telco-interop      # or: cargo install telco-interop --features cli
telco-interop read abbm.xml                  # what a file is, or the TEQ code to refuse it with
telco-interop check va.xml                   # the validation issues, with rule and source
telco-interop code 8103                      # a Meldungscode in WBCI and S/PRI
telco-interop timeline --as ekpauf switch/   # one switch replayed: timeline, findings, deadlines
```

It prints no personal data and exits 1 on an error, a finding or an overdue obligation.
**[Guide](https://hupe1980.github.io/telco-interop/docs/start/command-line/)**

Need only one interface? Depend on `wbci` or `spri` directly; they are the same code.

Unofficial; not affiliated with or endorsed by the AK S&P, VATM, BUGLAS or Deutsche Telekom.

## License

Licensed under either of [Apache License, Version 2.0](LICENSE-APACHE) or
[MIT license](LICENSE-MIT) at your option.
