import { BN } from "@coral-xyz/anchor";
import { Transaction } from "@solana/web3.js";
import { createTransferInstruction, TOKEN_PROGRAM_ID } from "@solana/spl-token";
import { assert } from "chai";
import {
  bankrunContext,
  bankrunProgram,
  bankRunProvider,
  driftAccounts,
  driftGroup,
  DRIFT_USDC_SPOT_MARKET,
  ecosystem,
  groupAdmin,
  oracles,
  users,
} from "../../rootHooks";
import { getTokenBalance } from "../../utils/genericTests";
import { closeBank } from "../../utils/group-instructions";
import {
  makeAddDriftBankIx,
  makeInitDriftUserIx,
} from "../../utils/drift-instructions";
import {
  defaultDriftBankConfig,
  USDC_INIT_DEPOSIT_AMOUNT,
  USDC_MARKET_INDEX,
} from "../../utils/drift-utils";
import {
  deriveBankWithSeed,
  deriveFeeVault,
  deriveInsuranceVault,
  deriveLiquidityVault,
} from "../../utils/pdas";
import { processBankrunTransaction } from "../../utils/tools";

/**
 * Drift banks need no venue accounts to close: the bank's Drift user / user stats (and the seed
 * deposit in them) stay behind, and the vaults close like a regular bank's.
 */
describe("d17: Close Drift bank", () => {
  const seed = new BN(1701);
  const DONATION = 7;

  it("(admin) closes a Drift USDC bank and its vaults with no venue accounts", async () => {
    const mint = ecosystem.usdcMint.publicKey;
    const spotMarket = driftAccounts.get(DRIFT_USDC_SPOT_MARKET);
    const [bank] = deriveBankWithSeed(
      bankrunProgram.programId,
      driftGroup.publicKey,
      mint,
      seed
    );
    await processBankrunTransaction(
      bankrunContext,
      new Transaction().add(
        await makeAddDriftBankIx(
          groupAdmin.mrgnBankrunProgram,
          {
            group: driftGroup.publicKey,
            feePayer: groupAdmin.wallet.publicKey,
            bankMint: mint,
            integrationAcc1: spotMarket,
            oracle: oracles.usdcOracle.publicKey,
          },
          { config: defaultDriftBankConfig(oracles.usdcOracle.publicKey), seed }
        )
      ),
      [groupAdmin.wallet]
    );
    // Resolves its accounts from the bank, so it must land in a later tx.
    await processBankrunTransaction(
      bankrunContext,
      new Transaction().add(
        await makeInitDriftUserIx(
          groupAdmin.mrgnBankrunProgram,
          {
            feePayer: groupAdmin.wallet.publicKey,
            bank,
            signerTokenAccount: groupAdmin.usdcAccount,
          },
          { amount: USDC_INIT_DEPOSIT_AMOUNT },
          USDC_MARKET_INDEX
        )
      ),
      [groupAdmin.wallet]
    );
    const { integrationAcc2: driftUser, integrationAcc3: driftUserStats } =
      await bankrunProgram.account.bank.fetch(bank);

    const vaults = [
      deriveLiquidityVault,
      deriveInsuranceVault,
      deriveFeeVault,
    ].map((derive) => derive(bankrunProgram.programId, bank)[0]);
    // A donation proves the sweep runs on the venue-less path too.
    await processBankrunTransaction(
      bankrunContext,
      new Transaction().add(
        createTransferInstruction(
          users[0].usdcAccount,
          vaults[0],
          users[0].wallet.publicKey,
          DONATION
        )
      ),
      [users[0].wallet]
    );

    const closed = [bank, ...vaults];
    const infos = await Promise.all(
      closed.map((k) => bankRunProvider.connection.getAccountInfo(k))
    );
    infos.forEach((info) => assert.isNotNull(info));
    const rent = infos.reduce((sum, info) => sum + info.lamports, 0);
    const [lamportsBefore, tokensBefore] = await Promise.all([
      bankRunProvider.connection.getBalance(groupAdmin.wallet.publicKey),
      getTokenBalance(bankRunProvider, groupAdmin.usdcAccount),
    ]);

    await processBankrunTransaction(
      bankrunContext,
      new Transaction().add(
        await closeBank(groupAdmin.mrgnBankrunProgram, {
          marginfiGroup: driftGroup.publicKey,
          bank,
          admin: groupAdmin.wallet.publicKey,
          closeVaults: {
            tokenProgram: TOKEN_PROGRAM_ID,
            mint,
            adminTokenAccount: groupAdmin.usdcAccount,
          },
        })
      ),
      [groupAdmin.wallet]
    );

    for (const k of closed) {
      assert.isNull(await bankRunProvider.connection.getAccountInfo(k));
    }
    // Stranded on Drift's side, seed deposit included.
    for (const k of [driftUser, driftUserStats]) {
      assert.isNotNull(await bankRunProvider.connection.getAccountInfo(k));
    }

    const [lamportsAfter, tokensAfter] = await Promise.all([
      bankRunProvider.connection.getBalance(groupAdmin.wallet.publicKey),
      getTokenBalance(bankRunProvider, groupAdmin.usdcAccount),
    ]);
    assert.equal(tokensAfter - tokensBefore, DONATION);
    // Minus the tx fee.
    assert.isAtLeast(lamportsAfter - lamportsBefore, rent - 10_000);
  });
});
