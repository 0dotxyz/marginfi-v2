# Summary

Not sure how to pack our banks and oracles for Risk Engine checks? Read on!

## High Level Summary

For each Balance in the user's Account, pass (1) the bank, (2) the required Oracle accounts in
remaining accounts.

## Other Remaining Accounts

In some instances, you must also pack other accounts into remaining accounts.

### T22 Mints

If the Mint for a given Bank is Token22, pack it first, before all the risk accounts.

### Liquidate Oracles

For `lending_account_liquidate`, pack in the following order:

```
 * liab_mint_ai (if token2022 mint),
 * asset_oracle_ai,
 * liab_oracle_ai,
 * liquidator_observation_ais...,
 * liquidatee_observation_ais...,
```

Here's how this might look in practice (token A is the asset, USDC is the liability):
```
remaining: [
    tokenAOracle,
    usdcOracle,
    ...composeRemainingAccounts([
        [liabilityBankKey, usdcOracle],
        [assetBankKey, tokenAOracle],
    ]),
    ...composeRemainingAccounts([
        [liabilityBankKey, usdcOracle],
        [assetBankKey, tokenAOracle],
    ]),
]
```

## Sorting Requirement

Accounts must be sorted bitwise by Bank pubkey. Since 1.2, this is also the order they appear in the
Account's Balances.

Hint: If dealing with a very old account, try calling sort_balances (permissionless) on it.

### TS Example:

```
export const composeRemainingAccounts = (
  banksAndOracles: PublicKey[][]
): PublicKey[] => {
  banksAndOracles.sort((a, b) => {
    const A = a[0].toBytes();
    const B = b[0].toBytes();
    // find the first differing byte
    for (let i = 0; i < 32; i++) {
      if (A[i] !== B[i]) {
        // descending: bigger byte should come first
        return B[i] - A[i];
      }
    }
    return 0; // identical keys
  });

  // flatten out [bank, oracle…, oracle…] → [bank, oracle…, bank, oracle…, …]
  return banksAndOracles.flat();
};
```

### Rust Examples

Noting how Balances are sorted:

```
    fn sort_balances(&mut self) {
        // Sort all balances in descending order by bank_pk
        self.balances.sort_by(|a, b| b.bank_pk.cmp(&a.bank_pk));
    }
```

Assuming you already have the Balances and now have a list of Banks:

```
        // Load all banks
        let mut banks = vec![];
        for bank_pk in bank_pks.clone() {
            let bank = load_and_deserialize::<Bank>(self.ctx.clone(), &bank_pk).await;
            banks.push(bank);
        }

        // Bank -> AccountMetas
        let account_metas = banks
            .iter()
            .zip(bank_pks.iter())
            .flat_map(|(bank, bank_pk)| {
                // The bank is included for all oracle types
                let mut metas = vec![AccountMeta {
                    pubkey: *bank_pk,
                    is_signer: false,
                    is_writable: false,
                }];

                // Oracle meta is included for all but fixed-price banks
                if bank.config.oracle_setup != OracleSetup::Fixed {
                    let oracle_key = {
                        let oracle_key = bank.config.oracle_keys[0];
                        get_oracle_id_from_feed_id(oracle_key).unwrap_or(oracle_key)
                    };

                    metas.push(AccountMeta {
                        pubkey: oracle_key,
                        is_signer: false,
                        is_writable: false,
                    });

                    // For Kamino setups, also push keys[1]
                    // For Staked setup, also push keys[1] and keys[2]
                }
                metas
            })
            .collect::<Vec<_>>();
        account_metas
```

## Adding New Balances

Often, an instruction will add a new Balance. For example borrowing creates a Balance in the Bank we
are borrowing from: even though that balance doesn't exist yet, it must appear!

Let's say we have a user with one Balance: a deposit in a SOL bank. They want to deposit into the
Token A bank:

```
let remaining = composeRemainingAccounts([
    [solBank, solOracle], // we could read this off the user's Balances
    [tokenABank, tokenAOracle], // this we must add manually
]);

const oracleMeta: AccountMeta[] = remaining.map((pubkey) => ({
    pubkey,
    isSigner: false,
    isWritable: false,
}));

const ix = program.methods
.lendingAccountBorrow(amt)
.accounts({
    marginfiAccount: acc,
    bank: tokenABank,
    destinationTokenAccount: ata_a,
    tokenProgram: TOKEN_PROGRAM_ID,
})
.remainingAccounts(oracleMeta)
.instruction();
```

## Removing Balances

Sometimes, an instruction will remove a Balance. For example withdrawing an entire Balance removes
it.

Let's say we have a user with three balances: a deposit in a SOL Bank, a deposit in a USDC bank, and
a borrow from a Token A Bank. They want to withdraw all of their USDC.

```
let remaining = composeRemainingAccounts([
    [solBank, solOracle],
    [tokenABank, tokenAOracle],
    // We manually filter the bank to be withdrawn out!
    // [usdcBank, usdcOracle],
]);
```

## Oracle Crank and Refresh Requirements

For every switchboard OracleSetup you encounter in a user's balances, you must crank those oracles
before the risk TX lands.

```
    try {
      const swbProgram = await sb.AnchorUtils.loadProgramFromConnection(
        // @ts-ignore
        connection
      );

      const pullFeedInstances: sb.PullFeed[] = swbPullFeeds.map(
        (pubkey) => new sb.PullFeed(swbProgram, pubkey)
      );

      const gateway = await (process.env.CROSSBAR_API_URL
        ? pullFeedInstances[0].fetchGatewayUrl(
            new CrossbarClient(process.env.CROSSBAR_API_URL)
          )
        : pullFeedInstances[0].fetchGatewayUrl());

      const [pullIx, luts] = await sb.PullFeed.fetchUpdateManyIx(swbProgram, {
        feeds: pullFeedInstances,
        gateway,
        numSignatures: 1,
        payer: payer.publicKey,
      });

      const { blockhash, lastValidBlockHeight } =
        await connection.getLatestBlockhash();

      const v0Message = new TransactionMessage({
        payerKey: payer.publicKey,
        recentBlockhash: blockhash,
        instructions: pullIx,
      }).compileToV0Message(luts ?? []);

      const v0Tx = new VersionedTransaction(v0Message);
      v0Tx.sign([payer]);

      const signature = await connection.sendTransaction(v0Tx, {
        maxRetries: 5,
      });
      await connection.confirmTransaction(
        { signature, blockhash, lastValidBlockHeight },
        "confirmed"
      );

      console.log("Swb crank (v0) tx signature:", signature);
    } catch (err) {
      console.log("swb crank failed");
      console.log(err);
    }
```

For every Kamino OracleSetup, you must refresh the reserve. Typically these are small enough that might prepend them to the tx itself, since they must land in the same slot (though this will often require a LUT if the user has many Kamino positions):

```
  transaction.add(
    ...kaminoIxes,
    await lendingAccountWithdraw(...)
  );
```

Also keep in mind you have need to prepend a `createAssociatedTokenAccountIdempotentInstruction` if
performing a withdraw, and ComputeBudget instructions as needed.

## Putting It All Together

### TS Example

```
  let swbPullFeeds: PublicKey[] = [];
  const kaminoIxes: TransactionInstruction[] = [];
  let acc = await program.account.marginfiAccount.fetch(account);
  dumpAccBalances(acc);
  let balances = acc.lendingAccount.balances;
  let activeBalances: BankAndOracles[] = [];
  for (let i = 0; i < balances.length; i++) {
    const bal = balances[i];
    if (bal.active == 1) {
      const bankAcc = await program.account.bank.fetch(bal.bankPk);
      const setup = bankAcc.config.oracleSetup;
      const keys = bankAcc.config.oracleKeys;

      if ("switchboardPull" in setup) {
        const oracle = keys[0];
        console.log(`[${i}] swb oracle: ${oracle}`);
        swbPullFeeds.push(oracle);
        activeBalances.push([bal.bankPk, oracle]);
      } else if ("kaminoSwitchboardPull" in setup) {
        const oracle = keys[0];
        console.log(`[${i}] kamino swb oracle: ${oracle}`);
        console.log(`  extra key: ${keys[1]}`);
        swbPullFeeds.push(oracle); // still a switchboard feed
        activeBalances.push([bal.bankPk, oracle, keys[1]]);

        const kaminoReservePk: PublicKey = bankAcc.integrationAcc1;
        let reserve = await kaminoProgram.account.reserve.fetch(
          kaminoReservePk
        );
        const ix = await simpleRefreshReserve(
          kaminoProgram,
          kaminoReservePk,
          reserve.lendingMarket,
          reserve.config.tokenInfo.scopeConfiguration.priceFeed
        );
        kaminoIxes.push(ix);
      } else if ("pythPushOracle" in setup) {
        const oracle = keys[0];
        console.log(`[${i}] pyth oracle: ${oracle}`);
        activeBalances.push([bal.bankPk, oracle]);
      } else if ("kaminoPythPush" in setup) {
        const oracle = keys[0];
        console.log(`[${i}] kamino pyth oracle: ${oracle}`);
        console.log(`  extra key: ${keys[1]}`);
        activeBalances.push([bal.bankPk, oracle, keys[1]]);

        const kaminoReservePk: PublicKey = bankAcc.integrationAcc1;
        let reserve = await kaminoProgram.account.reserve.fetch(
          kaminoReservePk
        );
        const ix = await simpleRefreshReserve(
          kaminoProgram,
          kaminoReservePk,
          reserve.lendingMarket,
          reserve.config.tokenInfo.scopeConfiguration.priceFeed
        );
        kaminoIxes.push(ix);
      } else if ("stakedWithPythPush" in setup) {
        const oracle = keys[0];
        console.log(`[${i}] pyth oracle: ${oracle}`);
        console.log(`  lst pool/mint: ${keys[1]} ${keys[2]}`);
        activeBalances.push([bal.bankPk, oracle, keys[1], keys[2]]);
      } else if ("fixed" in setup) {
        // do nothing
      } else {
        const oracle = keys[0];
        console.log(`[${i}] other oracle: ${oracle}`);
        activeBalances.push([bal.bankPk, oracle]);
      }
    }
  }
```
where 
```
export const simpleRefreshReserve = (
  program: Program<KaminoLending>,
  reserve: PublicKey,
  market: PublicKey,
  oracle: PublicKey
) => {
  const ix = program.methods
    .refreshReserve()
    .accounts({
      reserve: reserve,
      lendingMarket: market,
      pythOracle: null,
      switchboardPriceOracle: null,
      switchboardTwapOracle: null,
      scopePrices: oracle,
    })
    .instruction();

  return ix;
};
```

Bearing in mind you will need to add (if creating a new Balance e.g. borrowing a new liability) or
remove (if getting rid of one e.g. withdrawing all of an asset) one more set of accounts as needed.
