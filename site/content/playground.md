+++
title = "Playground"
description = "Read and check a WBCI or S/PRI payload, or look up a Meldungscode, in your browser: the same answers as the telco-interop command line."
template = "playground.html"

[extra]
seo_title = "WBCI and S/PRI playground: read and check payloads in the browser · telco-interop"
+++

Paste a payload, drop a file or pick an example, then choose **Read** (what it is: interface,
kind, business case, ids, dates, codes) or **Check** (the validation issues, with rule, element,
source and the code the peer would answer with). The lines are exactly what
[`telco-interop read`, `check` and `code`](@/docs/start/command-line.md) print.

**Nothing leaves your browser.** The libraries run here as WebAssembly: no upload, no request
after the page has loaded, no cookies, no analytics. Like the command line, the result shows no
names, addresses, numbers or free text. The examples are synthetic.
