# Trident harness must be built with the exact Solana CLI that ships the
# platform-tools producing SBPF Trident's bundled runtime can load. Other
# patch versions (e.g. 3.1.14) emit ELF flags Trident rejects with "Program
# is not deployed". Keep this in lockstep with the `SOLANA_CLI_VERSION`
# job-level override in `.github/workflows/test.yaml`'s trident jobs.
REQUIRED_SOLANA := 3.1.13

MARGINFI_PROGRAM_ID := MFv2hWf31Z9kbCa1snEPYctwafyhdvnV7FZnsebVacA
MOCKS_PROGRAM_ID    := rec5EKMGg6MxZYaMdyBfgwp4d5rB9T1VQH5pJv5LtFJ

.PHONY: format clippy build check-solana install-solana types

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

# Regenerate `fuzz_0/types.rs` from the marginfi + mocks IDLs. The file is
# gitignored — every CI run regenerates it from the program source, and
# local development uses this target to bootstrap (or refresh) the
# harness whenever the on-chain IDL changes.
#
# Requires anchor 0.31.1 (the repo's pinned version); other anchor
# versions emit different IDL JSON and produce a different types.rs.
types:
	mkdir -p ../target/idl
	cd .. && anchor idl build -p marginfi -o target/idl/marginfi.json
	cd .. && anchor idl build -p mocks    -o target/idl/mocks.json
	# Only patch the TOP-LEVEL `"address"` field (2-space indent). A
	# bare `"address":` regex would also clobber every per-account
	# `address` (system_program at indent 10+, etc.) and bake the
	# marginfi program ID into every account-meta slot.
	sed -i '' 's|^  "address": "[^"]*"|  "address": "$(MARGINFI_PROGRAM_ID)"|' ../target/idl/marginfi.json
	sed -i '' 's|^  "address": "[^"]*"|  "address": "$(MOCKS_PROGRAM_ID)"|'    ../target/idl/mocks.json
	trident fuzz refresh fuzz_0 -s
