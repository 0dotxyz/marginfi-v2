import { BN } from "@coral-xyz/anchor";
import {
  ComputeBudgetProgram,
  PublicKey,
  Transaction,
} from "@solana/web3.js";
import {
  createTransferInstruction,
  TOKEN_PROGRAM_ID,
} from "@solana/spl-token";
import { assert } from "chai";
import {
  A_FARM_STATE,
  bankrunContext,
  bankrunProgram,
  bankRunProvider,
  ecosystem,
  farmAccounts,
  groupAdmin,
  kaminoAccounts,
  kaminoGroup,
  MARKET,
  oracles,
  TOKEN_A_RESERVE,
  USDC_RESERVE,
  users,
} from "../../rootHooks";
import {
  assertBankrunTxFailed,
  getTokenBalance,
} from "../../utils/genericTests";
import { closeBank } from "../../utils/group-instructions";
import {
  kaminoCloseBankAccounts,
  makeAddKaminoBankIx,
  makeInitObligationIx,
} from "../../utils/kamino-instructions";
import { defaultKaminoBankConfig } from "../../utils/kamino-utils";
import {
  deriveBankWithSeed,
  deriveBaseObligation,
  deriveFeeVault,
  deriveInsuranceVault,
  deriveLiquidityVault,
  deriveLiquidityVaultAuthority,
  deriveUserMetadata,
  deriveUserState,
} from "../../utils/pdas";
import { processBankrunTransaction } from "../../utils/tools";
import { FARMS_PROGRAM_ID, KLEND_PROGRAM_ID } from "../../utils/types";

const SEED_AMOUNT = 500;
const BANK_CANNOT_CLOSE = 6078;
const INVALID_KAMINO_RESERVE = 6213;

type Case = {
  name: string;
  seed: BN;
  mint: () => PublicKey;
  oracle: () => PublicKey;
  reserve: () => PublicKey;
  adminTokenAccount: () => PublicKey;
  userTokenAccount: () => PublicKey;
  farmState: () => PublicKey | null;
};

const usdc = (seed: number): Case => ({
  name: "USDC (no farm)",
  seed: new BN(seed),
  mint: () => ecosystem.usdcMint.publicKey,
  oracle: () => oracles.usdcOracle.publicKey,
  reserve: () => kaminoAccounts.get(USDC_RESERVE),
  adminTokenAccount: () => groupAdmin.usdcAccount,
  userTokenAccount: () => users[0].usdcAccount,
  farmState: () => null,
});

const tokenA = (seed: number): Case => ({
  name: "Token A (collateral farm)",
  seed: new BN(seed),
  mint: () => ecosystem.tokenAMint.publicKey,
  oracle: () => oracles.tokenAOracle.publicKey,
  reserve: () => kaminoAccounts.get(TOKEN_A_RESERVE),
  adminTokenAccount: () => groupAdmin.tokenAAccount,
  userTokenAccount: () => users[0].tokenAAccount,
  farmState: () => farmAccounts.get(A_FARM_STATE),
});

/** Adds a Kamino bank, seeds its obligation, and returns what the close needs. */
const setupBank = async (c: Case) => {
  const market = kaminoAccounts.get(MARKET);
  const reserve = c.reserve();
  const mint = c.mint();
  const [bank] = deriveBankWithSeed(
    bankrunProgram.programId,
    kaminoGroup.publicKey,
    mint,
    c.seed,
  );
  const [authority] = deriveLiquidityVaultAuthority(
    bankrunProgram.programId,
    bank,
  );
  const [obligation] = deriveBaseObligation(authority, market);
  const farmState = c.farmState();
  const obligationFarmUserState = farmState
    ? deriveUserState(FARMS_PROGRAM_ID, farmState, obligation)[0]
    : null;

  await processBankrunTransaction(
    bankrunContext,
    new Transaction().add(
      await makeAddKaminoBankIx(
        groupAdmin.mrgnBankrunProgram,
        {
          group: kaminoGroup.publicKey,
          feePayer: groupAdmin.wallet.publicKey,
          bankMint: mint,
          kaminoReserve: reserve,
          kaminoMarket: market,
          oracle: c.oracle(),
        },
        { config: defaultKaminoBankConfig(c.oracle()), seed: c.seed },
      ),
    ),
    [groupAdmin.wallet],
  );
  await processBankrunTransaction(
    bankrunContext,
    new Transaction().add(
      ComputeBudgetProgram.setComputeUnitLimit({ units: 2_000_000 }),
      await makeInitObligationIx(
        groupAdmin.mrgnBankrunProgram,
        {
          feePayer: groupAdmin.wallet.publicKey,
          bank,
          signerTokenAccount: c.adminTokenAccount(),
          lendingMarket: market,
          reserve,
          obligationFarmUserState,
          reserveFarmState: farmState,
        },
        new BN(SEED_AMOUNT),
      ),
    ),
    [groupAdmin.wallet],
  );

  const closeTx = async (
    closeReserve = reserve,
    adminTokenAccount = c.adminTokenAccount(),
  ) =>
    new Transaction().add(
      ComputeBudgetProgram.setComputeUnitLimit({ units: 1_400_000 }),
      await closeBank(groupAdmin.mrgnBankrunProgram, {
        marginfiGroup: kaminoGroup.publicKey,
        bank,
        admin: groupAdmin.wallet.publicKey,
        closeVaults: {
          tokenProgram: TOKEN_PROGRAM_ID,
          mint,
          adminTokenAccount,
          venueAccounts: kaminoCloseBankAccounts({
            marginfiProgramId: bankrunProgram.programId,
            bank,
            lendingMarket: market,
            reserve: closeReserve,
            obligationFarmUserState,
            reserveFarmState: farmState,
          }),
        },
      }),
    );

  return { bank, authority, obligation, obligationFarmUserState, closeTx };
};

const expectCloseFails = async (
  tx: Transaction,
  code: number,
  message?: string,
) => {
  const result = await processBankrunTransaction(
    bankrunContext,
    tx,
    [groupAdmin.wallet],
    true,
  );
  if (message) {
    assert.isTrue(
      "meta" in result &&
        result.meta.logMessages.some((log) => log.includes(message)),
      `expected log: ${message}`,
    );
  }
  assertBankrunTxFailed(result, code);
};

describe("k22: Close Kamino bank with its obligation", () => {
  const cases = [usdc(2201), tokenA(2202)];

  for (const c of cases) {
    it(`(admin) closes a ${c.name} bank, its vaults and obligation`, async () => {
      const { bank, authority, obligation, obligationFarmUserState, closeTx } =
        await setupBank(c);
      const [userMetadata] = deriveUserMetadata(KLEND_PROGRAM_ID, authority);

      const otherReserve = cases.find((o) => o !== c).reserve();
      await expectCloseFails(
        await closeTx(otherReserve),
        INVALID_KAMINO_RESERVE,
      );
      await expectCloseFails(
        await closeTx(undefined, c.userTokenAccount()),
        BANK_CANNOT_CLOSE,
        "admin_token_account must be owned by the admin",
      );

      const closed = [
        bank,
        obligation,
        ...[deriveLiquidityVault, deriveInsuranceVault, deriveFeeVault].map(
          (derive) => derive(bankrunProgram.programId, bank)[0],
        ),
      ];
      const infos = await Promise.all(
        closed.map((k) => bankRunProvider.connection.getAccountInfo(k)),
      );
      infos.forEach((info) => assert.isNotNull(info));
      const rent = infos.reduce((sum, info) => sum + info.lamports, 0);
      const [lamportsBefore, tokensBefore] = await Promise.all([
        bankRunProvider.connection.getBalance(groupAdmin.wallet.publicKey),
        getTokenBalance(bankRunProvider, c.adminTokenAccount()),
      ]);

      await processBankrunTransaction(bankrunContext, await closeTx(), [
        groupAdmin.wallet,
      ]);

      for (const k of closed) {
        assert.isNull(await bankRunProvider.connection.getAccountInfo(k));
      }
      // Not closable on Kamino's side.
      assert.isNotNull(
        await bankRunProvider.connection.getAccountInfo(userMetadata),
      );
      if (obligationFarmUserState) {
        assert.isNotNull(
          await bankRunProvider.connection.getAccountInfo(
            obligationFarmUserState,
          ),
        );
      }
      assert.equal(
        await bankRunProvider.connection.getBalance(authority),
        0,
        "obligation rent forwarded to the admin",
      );

      const [lamportsAfter, tokensAfter] = await Promise.all([
        bankRunProvider.connection.getBalance(groupAdmin.wallet.publicKey),
        getTokenBalance(bankRunProvider, c.adminTokenAccount()),
      ]);
      // Seed back, less Kamino rounding.
      assert.approximately(tokensAfter - tokensBefore, SEED_AMOUNT, 1);
      // Minus the tx fee.
      assert.isAtLeast(lamportsAfter - lamportsBefore, rent - 10_000);
    });
  }

  it("(admin) sweeps a liquidity vault donation along with the seed", async () => {
    const c = usdc(2203);
    const { bank, obligation, closeTx } = await setupBank(c);
    const [liquidityVault] = deriveLiquidityVault(
      bankrunProgram.programId,
      bank,
    );
    const DONATION = 7;
    await processBankrunTransaction(
      bankrunContext,
      new Transaction().add(
        createTransferInstruction(
          users[0].usdcAccount,
          liquidityVault,
          users[0].wallet.publicKey,
          DONATION,
        ),
      ),
      [users[0].wallet],
    );
    const tokensBefore = await getTokenBalance(
      bankRunProvider,
      c.adminTokenAccount(),
    );

    await processBankrunTransaction(bankrunContext, await closeTx(), [
      groupAdmin.wallet,
    ]);

    for (const k of [bank, obligation, liquidityVault]) {
      assert.isNull(await bankRunProvider.connection.getAccountInfo(k));
    }
    const tokensAfter = await getTokenBalance(
      bankRunProvider,
      c.adminTokenAccount(),
    );
    assert.approximately(tokensAfter - tokensBefore, SEED_AMOUNT + DONATION, 1);
  });
});
