+++
title = "S/PRI"
description = "The spri crate: S/PRI 4.x, the open-access interface between service provider and network owner: orders, messages, lifecycle and codes."
sort_by = "weight"
weight = 3
template = "section.html"

[extra]
h1 = "S/PRI 4.x"
seo_title = "S/PRI 4.x in Rust: open-access fibre orders · telco-interop"
+++

S/PRI is how a service provider orders lines from a fibre network owner and receives its
answers. The `spri` crate covers all of S/PRI 4.x (schema 4.00.03): the eight order business
cases, the twenty message types and the technical receipt, the order lifecycle of Anlage 7,
the 215 message codes and the [validation](@/docs/spri/validation.md) that predicts the network
owner's refusal before sending.

```sh
cargo add spri
```
