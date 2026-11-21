# Security

The crates parse XML sent by other companies; a bug in reading it is a security bug.

Security fixes target the latest published release.

Report a suspected vulnerability privately with **Report a vulnerability** under
[GitHub Security](https://github.com/hupe1980/telco-interop/security). Do not open a public
issue. Include the crate and version, a minimal reproducer and the impact.

Use synthetic data only: never send real customer names, addresses or phone numbers, real
carrier certificates or production payloads.

What the crates guarantee for untrusted input is listed under
[Security and hardening](https://hupe1980.github.io/telco-interop/docs/reference/security/): no
DTD or entity expansion, bounded input, no panics (fuzzed in CI), no personal data in errors
or `Debug`.
