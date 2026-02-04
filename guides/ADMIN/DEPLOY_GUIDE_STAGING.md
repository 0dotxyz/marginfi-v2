## DEPLOYING TO STAGING

Staging is different from mainnet only in that it uses a different key. Ensure that the key in lib.rs matches the intended staging key (typically stag8sTKds2h4KzjUw3zKTsxbqvT4XKHdaR9X9E6Rct)

### Requirements

- You have access to the staging program authority wallet (we will assume it is at `~/keys/staging-deploy.json` from here on)
- Anchor 0.31.1
- Solana 2.1.20

## Preferred Steps

- Build with `anchor build -p marginfi -- --no-default-features --features staging,custom-heap`
- Run:

```
solana program deploy --use-rpc \
  target/deploy/marginfi.so \
  --program-id stag8sTKds2h4KzjUw3zKTsxbqvT4XKHdaR9X9E6Rct \
  --keypair ~/keys/staging-deploy.json \
  --fee-payer ~/keys/staging-deploy.json \
  --url <your rpc (a paid RPC is likely required here)>
```

If this is your first time deploying, use the keypair.json in the target folder instead of the program's id for program-id

- Failed? That happens often. `solana program close --buffers -k ~/keys/staging-deploy.json` to recover the buffer funds and try again (Note: this costs you .02 SOL to try again)
- Still failing? That happens. Try to recover the buffer instead of closing it: `solana-keygen recover -o recovered-buffer.json` (then enter the buffer seed phrase). Then:

```
solana program deploy --use-rpc \
  target/deploy/marginfi.so \
  --program-id stag8sTKds2h4KzjUw3zKTsxbqvT4XKHdaR9X9E6Rct \
  --keypair ~/keys/staging-deploy.json \
  --fee-payer ~/keys/staging-deploy.json \
  --url <your rpc (https://api.mainnet-beta.solana.com is usually fine)> \
  --buffer recovered-buffer.json
```

- Program buffer full (e.g. `account data too small for instruction`)? Use:

```
solana program extend \
  --url <RPC_URL> \
  --keypair ~/keys/staging-deploy.json \
  stag8sTKds2h4KzjUw3zKTsxbqvT4XKHdaR9X9E6Rct 10000
```

- If you changed your wallet config, make sure to remove the staging wallet from your Solana config to avoid sausage fingers errors in the future: `solana config set --keypair ~/.config/solana/id.json`

### Alternative Steps With Anchor

- Note: this rarely works, the program is probably too chonky.
- Build with `anchor build -p marginfi -- --no-default-features --features staging,custom-heap`
- If this is your first time deploying (to a new key), with `anchor build -p marginfi -- --no-default-features --features staging,custom-heap`
- Ensure anchor.toml is configured like this:

```
[provider]
cluster = "https://api.mainnet-beta.solana.com"
wallet = "~/keys/staging-deploy.json"
```

Adjust the cluster as needed if using a custom rpc.

- Deploy with `anchor upgrade target/deploy/marginfi.so --program-id stag8sTKds2h4KzjUw3zKTsxbqvT4XKHdaR9X9E6Rct`. Use deploy instead upgrade if this is your first deployment, and use the keypair.json in the target folder instead of the program's id for program-id.

## Deploying the IDL

Check who the authority wallet is with:

```
anchor idl authority \
  stag8sTKds2h4KzjUw3zKTsxbqvT4XKHdaR9X9E6Rct \
  --provider.cluster <your rpc (https://api.mainnet-beta.solana.com is usually fine)> \
  --provider.wallet   ~/keys/staging-deploy.json
```

We'll assume the authority is `~/keys/staging-deploy.json`.

If this is not the first staging deploy, use:

```
anchor idl upgrade \
  stag8sTKds2h4KzjUw3zKTsxbqvT4XKHdaR9X9E6Rct \
  -f target/idl/marginfi.json \
  --provider.cluster <your rpc (a paid RPC is likely required here)> \
  --provider.wallet   ~/keys/staging-deploy.json
```

If this is the first IDl deploy, run:

```
anchor idl init \
  stag8sTKds2h4KzjUw3zKTsxbqvT4XKHdaR9X9E6Rct \
  -f target/idl/marginfi.json \
  --provider.cluster <your rpc (a paid RPC is likely required here)> \
  --provider.wallet   ~/keys/staging-deploy.json
```

## Deploying Staked Collateral to Staging

Note: Generally, don't bother doing this. Just use the actual mainnet deployment of the program at `SVSPxpvHdN29nkVg9rPapPNDddN5DipNLRUFhyjFThE`, maintained by the Solana Foundation. If for some reason you don't want to, read on.

The Staked Collateral feature uses spl-single-pool, developed by the Solana Foundation (https://github.com/solana-labs/solana-program-library/tree/master/single-pool). This guide will show you how to deploy that program.

First you will need:

- Agave tools 2.1.0 or later (`sh -c "$(curl -sSfL https://release.anza.xyz/stable/install)"`) and possibly `agave-install init 2.1.0`
- A wallet with at least 2 SOL (this guide will assume your wallet is at `~/keys/staging-deploy.json`). Verify the pubkey of your wallet with `solana-keygen pubkey ~/keys/staging-deploy.json` and verify you have at least 2 SOL with `solana balance -k ~/keys/staging-deploy.json`
- An RPC provider connected to mainnet (`solana config set --url https://api.mainnet-beta.solana.com`). The solana public api is usually fine.

Steps:

- Clone https://github.com/solana-labs/solana-program-library/tree/master/single-pool and pull latest
- Navigate to programs/single-pool and run `cargo build-sbf`
- Navigate back up to root, then navigate to target. Verify that `solana-keygen pubkey deploy/spl_single_pool-keypair.json` matches the program's declared id. If you want to generate a new id, delete this file and build again to generate a new program keypair. Don't forget to update the declare_id in lib.rs as needed.
- Deploy the program with:

```
solana program deploy \
  deploy/spl_single_pool.so \
  --program-id deploy/spl_single_pool-keypair.json \
  --keypair ~/keys/staging-deploy.json \
  --fee-payer ~/keys/staging-deploy.json \
  --url <your_rpc_url (optional, omit this line to use api.mainnet-beta)>
```

## Deploying the Alternative Staging Program

A second deployment of the staging program is used for internal testing e.g. the Kamino integration feature.

Follow the same steps as `Preferred Steps`, except you build with `--features stagingalt,custom-heap` and use key
`5UDghkpgW1HfYSrmEj2iAApHShqU44H6PKTAar9LL9bY` for the program id.
