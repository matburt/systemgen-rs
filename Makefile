.PHONY: check test hooks survey

check:
	cargo fmt --all --check
	cargo clippy --all-targets --all-features -- -D warnings
	cargo test --all-features

test:
	cargo test --all-features

hooks:
	git config core.hooksPath .githooks

survey:
	cargo run --release --example survey -p systemgen
