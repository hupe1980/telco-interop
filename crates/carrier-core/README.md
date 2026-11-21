# carrier-core

**The shared foundation of the [telco-interop](https://github.com/hupe1980/telco-interop)
carrier interfaces. You don't depend on it directly.**

[![crates.io](https://img.shields.io/crates/v/carrier-core.svg)](https://crates.io/crates/carrier-core)
[![docs.rs](https://docs.rs/carrier-core/badge.svg)](https://docs.rs/carrier-core)

Use [`wbci`](https://crates.io/crates/wbci), [`spri`](https://crates.io/crates/spri) or
[`telco-interop`](https://crates.io/crates/telco-interop); they re-export what you need as
their own API. This crate holds what WBCI and S/PRI share, so a fix in one is a fix in both:

| Module | What |
|---|---|
| `text` | schema facets (`TextSpec`, `Charset`, whitespace), the `text_type!` and `code_enum!` macros; `ValueError`, which never contains the rejected value |
| `id` | `CarrierCode`, `VorabstimmungsId`, `LineId` |
| `calendar` | German working days: nationwide public holidays (Easter computed), Mon–Fri or Mon–Sat, closing days |
| `count` | working-day and working-hour arithmetic with an `Explanation` a person can check by hand |
| `envelope` | `Zeitstempel`, `SignaturId`, `SignaturText` |
| `xml` | a bounded tree parser that refuses DTDs and resolves only the five predefined entities, a cursor that walks elements in schema order, an indenting writer |

No I/O, no clock, no global state. The API serves the sibling crates and is not stable on its
own.

Unofficial; not affiliated with or endorsed by the AK S&P, VATM or BUGLAS.

## License

Licensed under either of [Apache License, Version 2.0](LICENSE-APACHE) or
[MIT license](LICENSE-MIT) at your option.
