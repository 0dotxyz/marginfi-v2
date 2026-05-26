# Trident harness must be built with the exact Solana CLI that ships the
# platform-tools producing SBPF Trident's bundled runtime can load. Other
# patch versions (e.g. 3.1.14) emit ELF flags Trident rejects with "Program
# is not deployed". Keep this in lockstep with `SOLANA_VERSION` in
# `.github/workflows/trident-fuzz.yaml`.
REQUIRED_SOLANA := 3.1.13

.PHONY: format clippy build check-solana install-solana

format:
	cargo +nightly fmt

clippy:
	cargo clippy

# Verify the installed Solana CLI matches what Trident expects. Errors with
# a clear message if not — does NOT install anything. Run `make install-solana`
# explicitly if you want the install.
check-solana:
	@actual=$$(solana --version 2>/dev/null | awk '{print $$2}'); \
	if [ "$$actual" != "$(REQUIRED_SOLANA)" ]; then \
	  echo "❌ Solana CLI $(REQUIRED_SOLANA) required for Trident (you have: $${actual:-<not installed>})."; \
	  echo "   Run: make install-solana"; \
	  exit 1; \
	fi

# Opt-in: download and install Solana CLI at the required version. Idempotent
# (overwrites any existing install).
install-solana:
	sh -c "$$(curl -sSfL https://release.anza.xyz/v$(REQUIRED_SOLANA)/install)"

build: check-solana
	cargo build-sbf --manifest-path ../programs/marginfi/Cargo.toml --arch v2
