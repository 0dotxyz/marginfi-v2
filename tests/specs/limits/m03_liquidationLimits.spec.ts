import { BN } from "@coral-xyz/anchor";
import {
  createAssociatedTokenAccountIdempotentInstruction,
  getAssociatedTokenAddressSync,
} from "@solana/spl-token";
import {
  ComputeBudgetProgram,
  Keypair,
  PublicKey,
  Transaction,
  TransactionInstruction,
  TransactionMessage,
  VersionedTransaction,
} from "@solana/web3.js";
import { assert } from "chai";
import { bigNumberToWrappedI80F48 } from "@mrgnlabs/mrgn-common";
import {
  bankrunContext,
  bankrunProgram,
  banksClient,
  globalProgramAdmin,
  groupAdmin,
  klendBankrunProgram,
  oracles,
  users,
} from "../../rootHooks";
import {
  configureBank,
  configureBankOracle,
} from "../../utils/group-instructions";
import {
  blankBankConfigOptRaw,
  MAX_BALANCES,
  ORACLE_SETUP_JUPLEND_LST,
  ORACLE_SETUP_JUPLEND_MSOL,
  ORACLE_SETUP_KAMINO_LST,
  ORACLE_SETUP_KAMINO_MSOL,
} from "../../utils/types";
import {
  borrowIx,
  composeRemainingAccounts,
  composeRemainingAccountsMetaBanksOnly,
  composeRemainingAccountsWriteableMeta,
  depositIx,
  endLiquidationIx,
  initLiquidationRecordIx,
  liquidateIx,
  repayIx,
  startLiquidationIx,
  withdrawIx,
} from "../../utils/user-instructions";
import {
  countTxAccountLocks,
  createLookupTableForInstructions,
  getBankrunBlockhash,
  mintToTokenAccount,
  processBankrunTransaction,
  processBankrunV0Transaction,
  quiet,
  TX_ACCOUNT_LOCK_LIMIT,
} from "../../utils/tools";
import { genericMultiBankTestSetup } from "../../genericSetups";
import { refreshPullOraclesBankrun } from "../../utils/bankrun-oracles";
import { createMintBankrun } from "../../utils/mocks";
import {
  defaultKaminoBankConfig,
  simpleRefreshObligation,
  simpleRefreshReserve,
} from "../../utils/kamino-utils";
import {
  createKaminoMarket,
  createReserve,
} from "../../utils/kamino-reserve-setup";
import {
  makeAddKaminoBankIx,
  makeInitObligationIx,
  makeKaminoDepositIx,
} from "../../utils/kamino-instructions";
import {
  configureJuplendProtocolPermissions,
  initJuplendGlobals,
  initJuplendPool,
} from "../../utils/juplend/jlr-pool-setup";
import {
  addJuplendBankIx,
  makeJuplendInitPositionIx,
} from "../../utils/juplend/group-instructions";
import { makeJuplendDepositIx } from "../../utils/juplend/user-instructions";
import { refreshJupSimple } from "../../utils/juplend/shorthand-instructions";
import { getJuplendPrograms } from "../../utils/juplend/programs";
import {
  defaultJuplendBankConfig,
  DEFAULT_BORROW_CONFIG_MIN,
  JuplendPoolKeys,
} from "../../utils/juplend/types";
import { deriveJuplendGlobalKeys } from "../../utils/juplend/juplend-pdas";
import {
  deriveBankWithSeed,
  deriveBaseObligation,
  deriveLiquidityVaultAuthority,
} from "../../utils/pdas";

/** Mirrors the program's `MAX_COSTLY_POSITIONS`. */
const CAP = 4;
/** Swept past the cap so the log still shows where each path actually tops out. */
const PROBE = 8;
const DECIMALS = 9;
const TOKENS = (n: number) => new BN(n * 10 ** DECIMALS);

/** Real mainnet pricing accounts loaded at genesis; cloned with the bank's mint patched in. */
const SANCTUM_SPL_POOL = new PublicKey(
  "9mhGNSPArRMHpLDMSmxAvuoizBqtBGqYdT8WGuqgxNdn",
);
const MSOL_STATE = new PublicKey(
  "8szGkuLTAux9XMgZ2vtY39jVSowEcpBfFfD8hXSEqdGC",
);

type Venue = "kamino" | "juplend";

const VENUES: Array<{ venue: Venue; groupSeed: string }> = [
  { venue: "juplend", groupSeed: "MARGINFI_GROUP_SEED_12340000M300" },
  { venue: "kamino", groupSeed: "MARGINFI_GROUP_SEED_12340000M301" },
];

type VenueBank = {
  bank: PublicKey;
  mint: PublicKey;
  reserve?: PublicKey;
  pool?: JuplendPoolKeys;
  /** bank, SOL/USD feed, venue state, LST/mSOL pricing account. */
  group: PublicKey[];
};

VENUES.forEach(({ venue, groupSeed }) => {
  describe(`m03: Liquidation limits, ${venue} collateral + native (LST/mSOL setups)`, () => {
    const ACCOUNT = `throwaway_account_m3_${venue}`;
    const venueBanks: VenueBank[] = [];
    let debtBank: PublicKey;
    let debtGroup: PublicKey[];
    /** The seized position. A capped account always holds plain collateral to take instead. */
    let nativeBank: PublicKey;
    let nativeGroup: PublicKey[];
    let market: PublicKey;

    const liquidatee = () => users[0];
    const liquidator = () => groupAdmin;
    const ata = (mint: PublicKey, owner: PublicKey) =>
      getAssociatedTokenAddressSync(mint, owner);
    const wsolOracle = () => oracles.wsolOracle.publicKey;
    const active = (n: number) => venueBanks.slice(0, n);
    let padGroups: PublicKey[][] = [];
    const liquidateeGroups = (n: number) => [
      ...active(n).map((b) => b.group),
      nativeGroup,
      ...padGroups,
      debtGroup,
    ];
    /** Fills the account to its 16-balance maximum, each pad bank on its own oracle. */
    const padToFullAccount = (n: number) => {
      padGroups = Array.from({ length: MAX_BALANCES - 2 - n }, () => [
        PublicKey.unique(),
        PublicKey.unique(),
      ]);
    };
    const obligationOf = (b: VenueBank) =>
      deriveBaseObligation(
        deriveLiquidityVaultAuthority(bankrunProgram.programId, b.bank)[0],
        market,
      )[0];

    /** Venue state the health check reads must be fresh in the same slot. */
    const refreshIxs = async (n: number): Promise<TransactionInstruction[]> =>
      venue === "kamino"
        ? [
            await klendBankrunProgram.methods
              .refreshReservesBatch(true)
              .remainingAccounts(
                active(n).flatMap((b) => [
                  { pubkey: b.reserve!, isSigner: false, isWritable: true },
                  { pubkey: market, isSigner: false, isWritable: false },
                ]),
              )
              .instruction(),
          ]
        : Promise.all(
            active(n).map((b) =>
              refreshJupSimple(getJuplendPrograms().lending, { pool: b.pool! }),
            ),
          );

    const classicIxs = async (n: number, bundled = true) => {
      const liquidateeAccounts = composeRemainingAccounts(liquidateeGroups(n));
      const liquidatorAccounts = composeRemainingAccounts([
        debtGroup,
        nativeGroup,
      ]);
      return [
        ComputeBudgetProgram.setComputeUnitLimit({ units: 1_400_000 }),
        ...(bundled ? await refreshIxs(n) : []),
        await liquidateIx(liquidator().mrgnBankrunProgram, {
          assetBankKey: nativeBank,
          liabilityBankKey: debtBank,
          liquidatorMarginfiAccount: liquidator().accounts.get(ACCOUNT),
          liquidateeMarginfiAccount: liquidatee().accounts.get(ACCOUNT),
          remaining: [
            ...nativeGroup.slice(1),
            ...debtGroup.slice(1),
            ...liquidatorAccounts,
            ...liquidateeAccounts,
          ],
          amount: TOKENS(0.1),
          liquidateeAccounts: liquidateeAccounts.length,
          liquidatorAccounts: liquidatorAccounts.length,
        }),
      ];
    };

    const receivershipIxs = async (n: number, bundled = true) => {
      const program = liquidator().mrgnBankrunProgram;
      const marginfiAccount = liquidatee().accounts.get(ACCOUNT);
      const groups = liquidateeGroups(n);
      return [
        ComputeBudgetProgram.setComputeUnitLimit({ units: 1_400_000 }),
        ...(bundled ? await refreshIxs(n) : []),
        await startLiquidationIx(program, {
          marginfiAccount,
          liquidationReceiver: liquidator().wallet.publicKey,
          remaining: composeRemainingAccountsWriteableMeta(groups),
        }),
        await withdrawIx(program, {
          marginfiAccount,
          bank: nativeBank,
          tokenAccount: liquidator().lstAlphaAccount,
          remaining: nativeGroup,
          amount: TOKENS(0.01),
        }),
        await repayIx(program, {
          marginfiAccount,
          bank: debtBank,
          tokenAccount: liquidator().lstAlphaAccount,
          amount: TOKENS(0.01),
        }),
        await endLiquidationIx(program, {
          marginfiAccount,
          remaining: composeRemainingAccountsMetaBanksOnly(groups),
        }),
      ];
    };

    const sendV0 = async (ixs: TransactionInstruction[], signer: Keypair) => {
      const lut = await createLookupTableForInstructions(signer, ixs);
      await refreshPullOraclesBankrun(oracles, bankrunContext, banksClient);
      const message = new TransactionMessage({
        payerKey: signer.publicKey,
        recentBlockhash: await getBankrunBlockhash(bankrunContext),
        instructions: ixs,
      }).compileToV0Message([lut]);
      await processBankrunV0Transaction(
        bankrunContext,
        new VersionedTransaction(message),
        [signer],
        false,
        true,
      );
    };

    /** Refresh in its own transaction, then liquidate. Both must land in the same slot, so the
     * lookup table (which warps to activate, and which the refresh needs for size) is built first. */
    const sendSplit = async (ixs: TransactionInstruction[], n: number) => {
      const signer = liquidator().wallet;
      const refresh = await refreshIxs(n);
      const lut = await createLookupTableForInstructions(signer, [
        ...refresh,
        ...ixs,
      ]);
      await refreshPullOraclesBankrun(oracles, bankrunContext, banksClient);
      for (const batch of [refresh, ixs]) {
        const message = new TransactionMessage({
          payerKey: signer.publicKey,
          recentBlockhash: await getBankrunBlockhash(bankrunContext),
          instructions: batch,
        }).compileToV0Message([lut]);
        await processBankrunV0Transaction(
          bankrunContext,
          new VersionedTransaction(message),
          [signer],
          false,
          true,
        );
      }
    };

    /** Copies a real pricing account, repointing the mint it validates against at `mint`. */
    const cloneWithMint = async (
      source: PublicKey,
      mintOffset: number,
      mint: PublicKey,
    ) => {
      const acc = (await banksClient.getAccount(source))!;
      const data = Buffer.from(acc.data);
      mint.toBuffer().copy(data, mintOffset);
      const key = Keypair.generate().publicKey;
      bankrunContext.setAccount(key, { ...acc, data });
      return key;
    };

    const addVenueState = async (
      group: PublicKey,
      seed: BN,
      mint: PublicKey,
      out: VenueBank,
    ) => {
      const admin = liquidator();
      if (venue === "kamino") {
        const reserve = Keypair.generate();
        await createReserve(
          reserve,
          market,
          mint,
          `m03_reserve_${seed}`,
          DECIMALS,
          wsolOracle(),
          ata(mint, admin.wallet.publicKey),
        );
        out.reserve = reserve.publicKey;
        await processBankrunTransaction(
          bankrunContext,
          new Transaction().add(
            await simpleRefreshReserve(
              klendBankrunProgram,
              out.reserve,
              market,
              wsolOracle(),
            ),
            await makeAddKaminoBankIx(
              admin.mrgnBankrunProgram,
              {
                group,
                feePayer: admin.wallet.publicKey,
                bankMint: mint,
                kaminoReserve: out.reserve,
                kaminoMarket: market,
                oracle: wsolOracle(),
              },
              { config: defaultKaminoBankConfig(wsolOracle()), seed },
            ),
          ),
          [admin.wallet],
        );
        await processBankrunTransaction(
          bankrunContext,
          new Transaction().add(
            ComputeBudgetProgram.setComputeUnitLimit({ units: 1_400_000 }),
            await makeInitObligationIx(
              admin.mrgnBankrunProgram,
              {
                feePayer: admin.wallet.publicKey,
                bank: out.bank,
                signerTokenAccount: ata(mint, admin.wallet.publicKey),
                lendingMarket: market,
                reserve: out.reserve,
              },
              TOKENS(1),
            ),
          ),
          [admin.wallet],
        );
        return out.reserve;
      }

      const pool = await initJuplendPool({
        admin: admin.wallet,
        mint,
        symbol: `m03_${seed}`,
        decimals: DECIMALS,
      });
      await configureJuplendProtocolPermissions({
        admin: admin.wallet,
        mint,
        lending: pool.lending,
        rateModel: pool.rateModel,
        tokenReserve: pool.tokenReserve,
        supplyPositionOnLiquidity: pool.supplyPositionOnLiquidity,
        borrowPositionOnLiquidity: pool.borrowPositionOnLiquidity,
        tokenProgram: pool.tokenProgram,
        borrowConfig: DEFAULT_BORROW_CONFIG_MIN,
      });
      out.pool = pool;
      await processBankrunTransaction(
        bankrunContext,
        new Transaction().add(
          await addJuplendBankIx(admin.mrgnBankrunProgram, {
            group,
            feePayer: admin.wallet.publicKey,
            bankMint: mint,
            bankSeed: seed,
            oracle: wsolOracle(),
            jupLendingState: pool.lending,
            fTokenMint: pool.fTokenMint,
            config: defaultJuplendBankConfig(wsolOracle(), DECIMALS),
            tokenProgram: pool.tokenProgram,
          }),
        ),
        [admin.wallet],
      );
      await processBankrunTransaction(
        bankrunContext,
        new Transaction().add(
          createAssociatedTokenAccountIdempotentInstruction(
            admin.wallet.publicKey,
            (
              await bankrunProgram.account.bank.fetch(out.bank)
            ).integrationAcc3,
            deriveLiquidityVaultAuthority(
              bankrunProgram.programId,
              out.bank,
            )[0],
            mint,
            pool.tokenProgram,
          ),
          await makeJuplendInitPositionIx(admin.mrgnBankrunProgram, {
            feePayer: admin.wallet.publicKey,
            signerTokenAccount: ata(mint, admin.wallet.publicKey),
            bank: out.bank,
            pool,
            seedDepositAmount: TOKENS(1),
            tokenProgram: pool.tokenProgram,
          }),
        ),
        [admin.wallet],
      );
      return pool.lending;
    };

    const addVenueBank = async (group: PublicKey, seed: BN, isLst: boolean) => {
      const admin = liquidator();
      const mintKp = Keypair.generate();
      const mint = mintKp.publicKey;
      await createMintBankrun(
        bankrunContext,
        globalProgramAdmin.wallet,
        DECIMALS,
        mintKp,
      );
      for (const owner of [
        admin.wallet.publicKey,
        liquidatee().wallet.publicKey,
      ]) {
        await processBankrunTransaction(
          bankrunContext,
          new Transaction().add(
            createAssociatedTokenAccountIdempotentInstruction(
              admin.wallet.publicKey,
              ata(mint, owner),
              owner,
              mint,
            ),
          ),
          [admin.wallet],
        );
        await mintToTokenAccount(mint, ata(mint, owner), TOKENS(1_000));
      }

      const [bank] = deriveBankWithSeed(
        bankrunProgram.programId,
        group,
        mint,
        seed,
      );
      const out: VenueBank = { bank, mint, group: [] };
      const venueState = await addVenueState(group, seed, mint, out);

      const pricing = isLst
        ? await cloneWithMint(SANCTUM_SPL_POOL, 162, mint)
        : await cloneWithMint(MSOL_STATE, 8, mint);
      const type =
        venue === "kamino"
          ? isLst
            ? ORACLE_SETUP_KAMINO_LST
            : ORACLE_SETUP_KAMINO_MSOL
          : isLst
          ? ORACLE_SETUP_JUPLEND_LST
          : ORACLE_SETUP_JUPLEND_MSOL;
      await processBankrunTransaction(
        bankrunContext,
        new Transaction().add(
          await configureBankOracle(admin.mrgnBankrunProgram, {
            bank,
            type,
            oracle: wsolOracle(),
            remaining: [venueState, pricing],
          }),
        ),
        [admin.wallet],
      );

      out.group = [bank, wsolOracle(), venueState, pricing];
      venueBanks.push(out);
    };

    const venueDeposit = async (b: VenueBank, amount: BN) => {
      const user = liquidatee();
      const marginfiAccount = user.accounts.get(ACCOUNT);
      const source = ata(b.mint, user.wallet.publicKey);
      const ixs =
        venue === "kamino"
          ? [
              await simpleRefreshReserve(
                klendBankrunProgram,
                b.reserve!,
                market,
                wsolOracle(),
              ),
              await simpleRefreshObligation(
                klendBankrunProgram,
                market,
                obligationOf(b),
                [b.reserve!],
              ),
              await makeKaminoDepositIx(
                user.mrgnBankrunProgram,
                {
                  marginfiAccount,
                  bank: b.bank,
                  signerTokenAccount: source,
                  lendingMarket: market,
                  reserve: b.reserve!,
                },
                amount,
              ),
            ]
          : [
              await makeJuplendDepositIx(user.mrgnBankrunProgram, {
                marginfiAccount,
                signerTokenAccount: source,
                bank: b.bank,
                pool: b.pool!,
                amount,
              }),
            ];
      await processBankrunTransaction(
        bankrunContext,
        new Transaction().add(...ixs),
        [user.wallet],
        false,
        true,
      );
    };

    before(async () => {
      market = await createKaminoMarket(Array(32).fill(0));
      const globals = deriveJuplendGlobalKeys();
      if (!(await banksClient.getAccount(globals.liquidity)))
        await initJuplendGlobals({ admin: groupAdmin.wallet });
    });

    it(`(admin) inits a debt bank, a plain bank and ${CAP} unique ${venue} LST/mSOL banks`, async () => {
      await refreshPullOraclesBankrun(oracles, bankrunContext, banksClient);
      const admin = liquidator();
      const { banks, throwawayGroup } = await quiet(() =>
        genericMultiBankTestSetup(2, ACCOUNT, Buffer.from(groupSeed), 1_000),
      );
      debtBank = banks[0];
      nativeBank = banks[1];
      debtGroup = [debtBank, oracles.pythPullLst.publicKey];
      nativeGroup = [nativeBank, oracles.pythPullLst.publicKey];
      await processBankrunTransaction(
        bankrunContext,
        new Transaction().add(
          await depositIx(admin.mrgnBankrunProgram, {
            marginfiAccount: admin.accounts.get(ACCOUNT),
            bank: debtBank,
            tokenAccount: admin.lstAlphaAccount,
            amount: TOKENS(10),
          }),
        ),
        [admin.wallet],
      );

      await quiet(async () => {
        for (let i = 0; i < PROBE; i++)
          await addVenueBank(
            throwawayGroup.publicKey,
            new BN(1_001 + i),
            i % 2 === 0,
          );
      });
    });

    it("(user 0) fills the account to the venue cap and borrows the debt", async () => {
      const user = liquidatee();
      for (const b of active(CAP)) await venueDeposit(b, TOKENS(10));
      await processBankrunTransaction(
        bankrunContext,
        new Transaction().add(
          await depositIx(user.mrgnBankrunProgram, {
            marginfiAccount: user.accounts.get(ACCOUNT),
            bank: nativeBank,
            tokenAccount: user.lstAlphaAccount,
            amount: TOKENS(10),
          }),
        ),
        [user.wallet],
      );
      await sendV0(
        [
          ComputeBudgetProgram.setComputeUnitLimit({ units: 1_400_000 }),
          ...(await refreshIxs(CAP)),
          await borrowIx(user.mrgnBankrunProgram, {
            marginfiAccount: user.accounts.get(ACCOUNT),
            bank: debtBank,
            tokenAccount: user.lstAlphaAccount,
            remaining: composeRemainingAccounts(liquidateeGroups(CAP)),
            amount: TOKENS(1),
          }),
        ],
        user.wallet,
      );

      const config = blankBankConfigOptRaw();
      config.liabilityWeightInit = bigNumberToWrappedI80F48(210);
      config.liabilityWeightMaint = bigNumberToWrappedI80F48(200);
      await processBankrunTransaction(
        bankrunContext,
        new Transaction().add(
          await configureBank(liquidator().mrgnBankrunProgram, {
            bank: debtBank,
            bankConfigOpt: config,
          }),
        ),
        [liquidator().wallet],
      );
    });

    it("reports how many positions each liquidation path covers on a full account", async () => {
      const payer = liquidator().wallet.publicKey;
      const locks = async (ixs: TransactionInstruction[]) =>
        countTxAccountLocks(payer, ixs);
      const fits = { classic: 0, recv: 0, classicSplit: 0, recvSplit: 0 };
      let refreshAtCap = 0;
      console.log(
        `\n${venue} + native, ${MAX_BALANCES} balances, limit ${TX_ACCOUNT_LOCK_LIMIT} locks:`,
      );
      for (let n = 1; n <= PROBE; n++) {
        padToFullAccount(n);
        const row = {
          classic: await locks(await classicIxs(n)),
          recv: await locks(await receivershipIxs(n)),
          classicSplit: await locks(await classicIxs(n, false)),
          recvSplit: await locks(await receivershipIxs(n, false)),
        };
        const refresh = await locks(await refreshIxs(n));
        if (n === CAP) refreshAtCap = refresh;
        console.log(
          `  ${String(n).padStart(2)} + ${MAX_BALANCES - n}: classic ${String(
            row.classic,
          ).padStart(2)} (${String(row.classicSplit).padStart(
            2,
          )} split), receivership ${String(row.recv).padStart(2)} (${String(
            row.recvSplit,
          ).padStart(2)} split), refresh tx ${refresh}`,
        );
        for (const k of Object.keys(fits) as Array<keyof typeof fits>)
          if (fits[k] === n - 1 && row[k] <= TX_ACCOUNT_LOCK_LIMIT) fits[k] = n;
      }
      padGroups = [];
      console.log(
        `  covers: classic ${fits.classic}, receivership ${fits.recv}; with the refresh split out ${fits.classicSplit} and ${fits.recvSplit} (cap ${CAP})`,
      );
      assert.isAtLeast(
        fits.classic,
        CAP,
        "classic no longer covers the cap in one transaction",
      );
      assert.isAtLeast(
        fits.recvSplit,
        CAP,
        "receivership no longer covers the cap, even with the refresh split out",
      );
      assert.isAtMost(
        refreshAtCap,
        TX_ACCOUNT_LOCK_LIMIT,
        "the refresh no longer fits one transaction",
      );
    });

    it("(admin) classic liquidation, refresh in its own transaction", async () => {
      await sendSplit(await classicIxs(CAP, false), CAP);
    });

    it("(admin) receivership, refresh in its own transaction", async () => {
      await processBankrunTransaction(
        bankrunContext,
        new Transaction().add(
          await initLiquidationRecordIx(liquidator().mrgnBankrunProgram, {
            marginfiAccount: liquidatee().accounts.get(ACCOUNT),
            feePayer: liquidator().wallet.publicKey,
          }),
        ),
        [liquidator().wallet],
      );
      await sendSplit(await receivershipIxs(CAP, false), CAP);
    });
  });
});
