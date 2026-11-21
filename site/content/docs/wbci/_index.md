+++
title = "WBCI"
description = "The wbci crate: WBCI 2.0, the provider-to-provider interface of the § 59 TKG switch and number porting, from request to deadline."
sort_by = "weight"
weight = 2
template = "section.html"

[extra]
h1 = "WBCI 2.0"
seo_title = "WBCI 2.0 in Rust: provider switch and number porting · telco-interop"
+++

WBCI is how the gaining and the losing provider agree a switch before any line is touched.
The `wbci` crate covers the whole interface: all six requests and six messages, the XML, the
validation rules, every deadline of Anlage 1 §2.5–§2.11, the process per pre-agreement, the
losing provider's check and the 36 message codes.

```sh
cargo add wbci
```
