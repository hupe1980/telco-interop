# telco-interop — task runner (https://just.systems)

default:
    @just --list --unsorted

# Everything CI runs, except fuzzing.
ci: fmt-check boundary lint test msrv deny package-check site-check playground-check
    @echo "CI gate passed."

# No planning ids and no unpublished paths in published files.
boundary:
    bash scripts/boundary-check.sh

fmt:
    cargo fmt --all

fmt-check:
    cargo fmt --all --check

# Clippy (pedantic, panics denied outside tests) and rustdoc, warnings are errors.
lint:
    cargo clippy --workspace --all-targets -- -D warnings
    cargo clippy -p telco-interop --all-targets --features cli -- -D warnings
    RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps

test *ARGS:
    cargo test --workspace --all-targets {{ ARGS }}
    cargo test -p telco-interop --features cli --all-targets {{ ARGS }}
    cargo test --workspace --doc

# The declared MSRV (rustup toolchain install 1.88).
msrv:
    cargo +1.88 test --workspace --all-targets
    cargo +1.88 test -p telco-interop --features cli --all-targets

deny:
    cargo deny check

# No AK S&P document may ship in a package (AGENTS.md).
package-check:
    #!/usr/bin/env bash
    set -euo pipefail
    for crate in carrier-core wbci spri telco-interop wita-kit; do
      if cargo package -p "$crate" --list --allow-dirty | grep -Ei '\.(xsd|wsdl|pdf)$'; then
        echo "$crate would ship a specification document"; exit 1
      fi
    done

# Validate everything the writers produce against the official WBCI and S/PRI XSDs (fetched, pinned).
schema:
    bash scripts/fetch-wbci-xsd.sh
    WBCI_XSD_REQUIRED=1 cargo test -p wbci --test schema -- --nocapture
    bash scripts/fetch-spri-xsd.sh
    SPRI_XSD_REQUIRED=1 cargo test -p spri --test schema -- --nocapture

# Regenerate the golden XML fixtures after an intended writer change; review the diff.
bless:
    WBCI_BLESS=1 cargo test -p wbci --test roundtrip
    SPRI_BLESS=1 cargo test -p spri --test roundtrip

# Fuzz both readers (nightly + cargo-fuzz), seeded from the fixtures.
fuzz SECONDS="60":
    mkdir -p fuzz/corpus/read fuzz/corpus/read_spri
    cp crates/wbci/tests/fixtures/*.xml fuzz/corpus/read/
    cp crates/spri/tests/fixtures/*.xml fuzz/corpus/read_spri/
    cd fuzz && cargo +nightly fuzz run read -- -max_total_time={{ SECONDS }}
    cd fuzz && cargo +nightly fuzz run read_spri -- -max_total_time={{ SECONDS }}

example:
    cargo run -p wbci --example switch

# The site's playground module: telco_interop::report as WebAssembly (rustup target add wasm32-unknown-unknown).
playground:
    #!/usr/bin/env bash
    set -euo pipefail
    cargo clippy -p playground --target wasm32-unknown-unknown -- -D warnings
    cargo build -p playground --target wasm32-unknown-unknown --profile playground
    mkdir -p site/static/playground
    cp target/wasm32-unknown-unknown/playground/playground.wasm site/static/playground/playground.wasm
    size=$(wc -c < site/static/playground/playground.wasm | tr -d ' ')
    echo "playground.wasm: $size bytes"
    if [ "$size" -gt 1572864 ]; then echo "playground.wasm is larger than 1.5 MB"; exit 1; fi

# The playground module when the wasm32 target is installed; otherwise a note (the site workflow always builds it).
playground-check:
    #!/usr/bin/env bash
    set -euo pipefail
    if rustup target list --installed 2>/dev/null | grep -qx wasm32-unknown-unknown; then
      just playground
    else
      echo "skipped: the playground needs 'rustup target add wasm32-unknown-unknown'"
    fi

site: playground-check
    cd site && zola serve

# Zola check, and every Rust sample wrapped so rustdoc's hidden lines stay hidden on the site.
site-check:
    cd site && zola check
    ! grep -rn -B1 '^```rust' site/content | grep -E '^[^:]+-[0-9]+-' | grep -v '{% <rust> %}' | grep .
