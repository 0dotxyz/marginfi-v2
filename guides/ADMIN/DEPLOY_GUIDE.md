## MAINNET VERIFIED DEPLOY GUIDE

Marginfi program authority is managed by squads (https://app.squads.so/squads/J3oBkTkDXU3TcAggJEa3YeBZE5om5yNAdTtLVNXFD47/home) and uses verified builds.

First you will need:

- Agave/Solana tools 3.1.13 (`sh -c "$(curl -sSfL https://release.anza.xyz/v3.1.13/install)"`, then ensure `$HOME/.local/share/solana/install/active_release/bin` is on `PATH`)
- Anchor CLI 1.0.2 (`cargo install avm --git https://github.com/solana-foundation/anchor --locked`, then `avm install 1.0.2` and `avm use 1.0.2`)
- solana-verify (`cargo install solana-verify`)
- Docker (https://docs.docker.com/engine/install/ubuntu/)
- A wallet with at least 15 SOL (this guide will assume your wallet is at `~/keys/mainnet-deploy.json`). Verify the pubkey of your wallet with `solana-keygen pubkey ~/keys/mainnet-deploy.json` and verify you have at least 15 SOL with `solana balance -k ~/keys/mainnet-deploy.json`
- An RPC provider connected to mainnet (`solana config set --url https://api.mainnet-beta.solana.com`). The Solana public api is usually not sufficient, and a custom rpc is suggested.

Steps:

- Make sure you are on the appropriate release tag branch and you have pulled latest.
- Run `./scripts/build-program-verifiable.sh marginfi mainnet`. Other people signing on the multisig should also run this and validate that the hash matches.
- Deploy the buffer with `./scripts/deploy-buffer.sh marginfi <YOUR_RPC_ENDPOINT> ~/keys/mainnet-deploy.json`
- Go to squads, developers, programs, pick marginfi. The buffer address is the output of the previous command. The buffer refund is the public key of the wallet you have used so far (`solana-keygen pubkey ~/keys/mainnet-deploy.json` if you don't know it). Click next.
- Go back to your cli and paste the command Squads gave you in step 2. If this key is not the one used in your solana CLI, make sure it pass it with -k, e.g.:

```
solana program set-buffer-authority <BUFFER> --new-buffer-authority <MULTISIG> -k ~/keys/mainnet-deploy.json
```

- Remember to send any residual funds to a hardware wallet:

```
solana balance <DEPLOY_WALLET>
>> N SOL

solana transfer \
  --from ~/keys/mainnet-deploy.json \
  <HARDWARE_WALLET> \
  <N>>
```

- Back up the current working program somewhere with `solana -um program dump MFv2hWf31Z9kbCa1snEPYctwafyhdvnV7FZnsebVacA mfi_backup.so`
- Click the pending upgrade to start a vote.
- Execute after the vote passes.
- Update the IDL.

### Updating the IDL

Anchor `1.0.x` no longer updates the old legacy Anchor IDL account. IDLs are now stored with Solana
Program Metadata. The canonical IDL update must be executed by the program upgrade authority, which
is (as of July 2026) the Squads multisig `J3oBkTkDXU3TcAggJEa3YeBZE5om5yNAdTtLVNXFD47`.

WARN: Do not use `--non-canonical` for the production IDL. That creates third-party metadata rather
than the canonical program IDL.

(Optional) Make sure the IDL has the program address:
`"address": "MFv2hWf31Z9kbCa1snEPYctwafyhdvnV7FZnsebVacA",` (by default, this field is likely blank).

Set up the shell variables:

```
PROGRAM_ID=MFv2hWf31Z9kbCa1snEPYctwafyhdvnV7FZnsebVacA
MS=J3oBkTkDXU3TcAggJEa3YeBZE5om5yNAdTtLVNXFD47
RPC=<your paid rpc>
IDL=$PWD/target/idl/marginfi.json
PAYER=~/keys/staging-admin.json
```

Confirm the multisig is still the program upgrade authority:

```
solana program show $PROGRAM_ID -u $RPC
```

Create a Program Metadata buffer and upload the IDL contents into it. The staging admin pays rent
and temporarily owns the buffer:

```
npx --yes @solana-program/program-metadata@0.5.1 \
  create-buffer $IDL \
  --rpc $RPC \
  --keypair $PAYER
```

Save the printed buffer address:

```
BUFFER=<printed buffer address>
```

Transfer the buffer authority to the multisig:

```
npx --yes @solana-program/program-metadata@0.5.1 \
  set-buffer-authority $BUFFER \
  --new-authority $MS \
  --rpc $RPC \
  --keypair $PAYER
```

Export the canonical IDL write transaction for Squads. Import the printed base58 transaction into
Squads and execute it from the multisig:

```
npx --yes @solana-program/program-metadata@0.5.1 \
  write idl $PROGRAM_ID \
  --buffer $BUFFER \
  --close-buffer \
  --export $MS \
  --export-encoding base58 \
  --rpc $RPC \
  --keypair $PAYER
```

Before this executes, signers can check the contents of the buffer like so:
```
npx --yes @solana-program/program-metadata@0.5.1 \
  fetch-buffer $BUFFER \
  --output /tmp/marginfi.buffer.idl.json \
  --rpc $RPC
```

After the Squads transaction executes, verify the on-chain Program Metadata IDL:

```
npx --yes @solana-program/program-metadata@0.5.1 \
  fetch idl $PROGRAM_ID \
  --output /tmp/marginfi.program-metadata.idl.json \
  --rpc $RPC

diff -u $IDL /tmp/marginfi.program-metadata.idl.json
```

### Updating the Verified Build

Put up the verified build with:

```
solana-verify export-pda-tx https://github.com/mrgnlabs/marginfi-v2 --program-id MFv2hWf31Z9kbCa1snEPYctwafyhdvnV7FZnsebVacA --uploader J3oBkTkDXU3TcAggJEa3YeBZE5om5yNAdTtLVNXFD47 --encoding base58 --compute-unit-price 0 --library-name marginfi
```

When the vote is pending, note the buffer in the proposed TX and check the buffer hash with:
```
solana-verify get-buffer-hash $BUFFER
```

- After the vote passes, verify the build with:

```
solana-verify verify-from-repo \
  --remote \
  --program-id MFv2hWf31Z9kbCa1snEPYctwafyhdvnV7FZnsebVacA \
  --library-name marginfi \
  https://github.com/mrgnlabs/marginfi-v2
```

### Voters:

- Clone the branch being deployed (see the release tag the person who initated the upgrade has given you) and run:

```
./scripts/build-program-verifiable.sh marginfi mainnet
```

- Check that the program builds with the hash that the person who is deploying gave you. Check what characters other people have validated in Signal, post the next six characters of the hash to verify you have actually checked and aren't skipping this step out of laziness.
- Check that the buffer contains this hash too `solana-verify get-buffer-hash <Buffer Address>`.
- After the vote is executed and the contract is upgraded, check that the contract contains the same hash. For example for MFv2, this is `solana-verify get-program-hash MFv2hWf31Z9kbCa1snEPYctwafyhdvnV7FZnsebVacA`

### Known Issues:

```
Error: parse error: parse error: invalid Cargo.lock format version: `4`
```

Update solana-verify (`cargo install solana-verify`) or manually edit cargo.lock to change the
version from 3 to 4

```
Unable to find docker image for Solana version 3.1.13
```

Update `solana-verify` first. If no image exists for the exact toolchain version, do not silently
substitute another Solana version for a production verified build; coordinate the expected hash with
the other signers.

Failing to verify build due to new multisig? Execute:

```
solana-verify export-pda-tx https://github.com/mrgnlabs/marginfi-v2 --program-id MFv2hWf31Z9kbCa1snEPYctwafyhdvnV7FZnsebVacA --uploader J3oBkTkDXU3TcAggJEa3YeBZE5om5yNAdTtLVNXFD47 --encoding base58 --compute-unit-price 0
```

and sign with the MS.

## RECENT DEPLOY HASHES

Here we list recent deployments to staging/mainnet. The hash is always the first 6 chars of the hash generated with the mainnet verified build guide above (even for staging, this is the mainnet hash, not the hash on staging. Staging does not get a verified build.).

### STAGING

- 0.1.0: Jan 30, 2025 ~2:35pm ET -- Hash: a4dd3e7
- 0.1.1: Feb 7, 2025 ~8:15am ET -- Hash: 03455c
- 0.1.2: March 14, 2025 ~3:00pm ET -- Hash 65bbbe
- 0.1.3: April 29, 2025 ~2:00pm ET -- Hash 1d8130
- 0.1.3 memory hotfix: May 1, 2025 ~4:00pm ET -- (no hash)
- 0.1.4: July 25, 2025 ~7:00pm ET -- (no hash)
- 0.1.9 July ~11, 2026 -- (no hash)

### MAINNET

- 0.1.0-alpha mainnet on Fev 3, 2024 ~2:45ET -- Hash: ea5d15
- 0.1.1: Feb 17, 2025 ~3:00pm ET -- Hash: 03455c
- 0.1.2: April 14, 2025 ~1:00pm ET -- Hash 65bbbe
- 0.1.3: May 27, 2025 ~1:00pm ET -- Hash ae9adb7
- 0.1.4: July 28, 2025 ~1:00pm ET -- Hash 1229b8
- 0.1.4 (transfer hotfix): October 2, 2025 ~4:30 ET -- Hash 866e5a
- 0.1.5 Oct 10, 2025 ~9:00am ET -- Hash 4e7867
- 0.1.6 Dec 18, 2025 ~11:00am ET -- Hash 65e54c
- 0.1.6a Dec 18, 2025 ~11:00am ET -- Hash not recorded
- 0.1.6b Dec 26, 2025 ~5:00pm ET -- Hash not recorded
- 0.1.7 Jan 26, 2026 ~11:00am ET -- Hash 918bd0
- 0.1.8 April 3, 2026 -- Hash 166cbb
- 0.1.9 July 14, 2026 ~11am ET -- Hash 26dda5e
