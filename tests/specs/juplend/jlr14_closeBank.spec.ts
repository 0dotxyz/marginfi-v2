import { BN } from "@coral-xyz/anchor";
import {
  createMintToInstruction,
  TOKEN_PROGRAM_ID,
} from "@solana/spl-token";
import { ComputeBudgetProgram, Transaction } from "@solana/web3.js";
import { assert } from "chai";
import {
  bankrunContext,
  bankrunProgram,
  bankRunProvider,
  ecosystem,
  globalProgramAdmin,
  groupAdmin,
  juplendAccounts,
  oracles,
} from "../../rootHooks";
import { getTokenBalance } from "../../utils/genericTests";
import { closeBank } from "../../utils/group-instructions";
import { addJuplendBankIx, makeJuplendInitPositionIx } from "../../utils/juplend/group-instructions";
import {
  deriveJuplendMrgnAddresses,
  deriveJuplendPoolKeys,
  findJuplendClaimAccountPda,
} from "../../utils/juplend/juplend-pdas";
import { JUPLEND_STATE_KEYS } from "../../utils/juplend/test-state";
import { defaultJuplendBankConfig } from "../../utils/juplend/types";
import { juplendCloseBankAccounts } from "../../utils/juplend/user-instructions";
import { processBankrunTransaction } from "../../utils/tools";

const BANK_SEED = new BN(1401);
const SEED_DEPOSIT = new BN(1_000_000);

describe("jlr14: Close JupLend bank with its position", () => {
  it("(admin) closes a USDC JupLend bank, its vaults, fToken vault and intermediary ATA", async () => {
    const group = juplendAccounts.get(JUPLEND_STATE_KEYS.jlr01Group);
    const mint = ecosystem.usdcMint.publicKey;
    const pool = deriveJuplendPoolKeys({ mint });
    const addresses = deriveJuplendMrgnAddresses({
      mrgnProgramId: bankrunProgram.programId,
      group,
      bankMint: mint,
      bankSeed: BANK_SEED,
    });
    const [claimAccount] = findJuplendClaimAccountPda(
      addresses.liquidityVaultAuthority,
      mint,
    );

    await processBankrunTransaction(
      bankrunContext,
      new Transaction().add(
        createMintToInstruction(
          mint,
          groupAdmin.usdcAccount,
          globalProgramAdmin.wallet.publicKey,
          BigInt(SEED_DEPOSIT.toString()),
        ),
      ),
      [globalProgramAdmin.wallet],
    );
    await processBankrunTransaction(
      bankrunContext,
      new Transaction().add(
        await addJuplendBankIx(groupAdmin.mrgnBankrunProgram, {
          group,
          feePayer: groupAdmin.wallet.publicKey,
          bankMint: mint,
          bankSeed: BANK_SEED,
          oracle: oracles.usdcOracle.publicKey,
          jupLendingState: pool.lending,
          fTokenMint: pool.fTokenMint,
          config: defaultJuplendBankConfig(
            oracles.usdcOracle.publicKey,
            ecosystem.usdcDecimals,
          ),
          tokenProgram: pool.tokenProgram,
        }),
      ),
      [groupAdmin.wallet],
    );
    await processBankrunTransaction(
      bankrunContext,
      new Transaction().add(
        await makeJuplendInitPositionIx(groupAdmin.mrgnBankrunProgram, {
          feePayer: groupAdmin.wallet.publicKey,
          signerTokenAccount: groupAdmin.usdcAccount,
          bank: addresses.bank,
          pool,
          seedDepositAmount: SEED_DEPOSIT,
        }),
      ),
      [groupAdmin.wallet],
    );
    // No withdraws yet, so no intermediary ATA: the close creates it.
    assert.isNull(
      await bankRunProvider.connection.getAccountInfo(
        addresses.withdrawIntermediaryAta,
      ),
    );
    assert.isAbove(
      await getTokenBalance(bankRunProvider, addresses.fTokenVault),
      0,
    );

    const closed = [
      addresses.bank,
      addresses.liquidityVault,
      addresses.insuranceVault,
      addresses.feeVault,
      addresses.fTokenVault,
    ];
    const infos = await Promise.all(
      closed.map((k) => bankRunProvider.connection.getAccountInfo(k)),
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
        ComputeBudgetProgram.setComputeUnitLimit({ units: 1_400_000 }),
        await closeBank(groupAdmin.mrgnBankrunProgram, {
          marginfiGroup: group,
          bank: addresses.bank,
          admin: groupAdmin.wallet.publicKey,
          closeVaults: {
            tokenProgram: TOKEN_PROGRAM_ID,
            mint,
            adminTokenAccount: groupAdmin.usdcAccount,
            venueAccounts: juplendCloseBankAccounts({
              pool,
              fTokenVault: addresses.fTokenVault,
              withdrawIntermediaryAta: addresses.withdrawIntermediaryAta,
              claimAccount,
            }),
          },
        }),
      ),
      [groupAdmin.wallet],
      false,
      true,
    );

    for (const k of [...closed, addresses.withdrawIntermediaryAta]) {
      assert.isNull(await bankRunProvider.connection.getAccountInfo(k));
    }
    const [lamportsAfter, tokensAfter] = await Promise.all([
      bankRunProvider.connection.getBalance(groupAdmin.wallet.publicKey),
      getTokenBalance(bankRunProvider, groupAdmin.usdcAccount),
    ]);
    // Seed back, less JupLend rounding.
    assert.approximately(
      tokensAfter - tokensBefore,
      SEED_DEPOSIT.toNumber(),
      2,
    );
    // Minus the tx fee.
    assert.isAtLeast(lamportsAfter - lamportsBefore, rent - 10_000);
  });
});
