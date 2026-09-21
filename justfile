default:
    @just --list

fmt:
    cargo fmt --all

fix:
    cargo fmt --all
    cargo clippy --all-targets --fix --allow-dirty -- -D warnings

test *args:
    cargo nextest run {{args}}

lint:
    cargo fmt --all --check
    cargo clippy --all-targets -- -D warnings

typos:
    @command -v typos >/dev/null 2>&1 && typos || echo "typos not installed; skipping (nix develop provides it)"

deny:
    cargo deny check

check: lint test deny typos

snapshots:
    UPDATE_SNAPSHOTS=1 cargo nextest run -p doneyet-ux

watch repo="acme/api" *args:
    cargo run -q -p doneyet-cli -- watch {{repo}} {{args}}

runs repo="acme/api" *args:
    cargo run -q -p doneyet-cli -- runs {{repo}} {{args}}

dash repo="acme/api" *args:
    cargo run -q -p doneyet-cli -- dash {{repo}} {{args}}
