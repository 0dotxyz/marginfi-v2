import { BN } from "@coral-xyz/anchor";
import {
  createAssociatedTokenAccountIdempotentInstruction,
  getAssociatedTokenAddressSync,
} from "@solana/spl-token";
import {
  ComputeBudgetProgram,
  Keypair,
  LAMPORTS_PER_SOL,
  PublicKey,
  Transaction,
  TransactionInstruction,
  TransactionMessage,
  VersionedTransaction,
} from "@solana/web3.js";
import { assert } from "chai";
import { bigNumberToWrappedI80F48 } from "@mrgnlabs/mrgn-common";
import {
  bankRunProvider,
  bankrunContext,
  bankrunProgram,
  banksClient,
  createSplStakePoolBankrun,
  createValidatorBankrun,
  ecosystem,
  groupAdmin,
  oracles,
  users,
} from "../../rootHooks";
import { Validator } from "../../utils/mocks";
import {
  addBank,
  addBankPermissionless,
  configureBank,
  configureBankOracle,
  groupInitialize,
  initStakedSettings,
} from "../../utils/group-instructions";
import {
  accountInit,
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
  ASSET_TAG_SOL,
  defaultBankConfig,
  blankBankConfigOptRaw,
  MAX_BALANCES,
  defaultStakedInterestSettings,
  ORACLE_SETUP_PYTH_PUSH,
} from "../../utils/types";
import {
  countTxAccountLocks,
  createLookupTableForInstructions,
  getBankrunBlockhash,
  mintToTokenAccount,
  processBankrunTransaction,
  processBankrunV0Transaction,
  TX_ACCOUNT_LOCK_LIMIT,
} from "../../utils/tools";
import { deriveBankWithSeed } from "../../utils/pdas";
import { createStakeAccount, delegateStake } from "../../utils/stake-utils";
import { depositToSinglePoolIxes } from "../../utils/spl-staking-utils";
import { refreshPullOraclesBankrun } from "../../utils/bankrun-oracles";
import { getEpochAndSlot } from "../../utils/bankrunConnection";

/** Mirrors the program's `MAX_COSTLY_POSITIONS`, which caps staked positions too. */
const CAP = 4;
/** Swept past the cap so the log shows where each path actually tops out. */
const STAKED_BANKS = 15;
const ACCOUNT = "m05_account";
const SOL = (n: number) => new BN(n * LAMPORTS_PER_SOL);

describe("m05: Liquidation limits with staked collateral (15 staked + SOL debt)", () => {
  const group = Keypair.fromSeed(
    Buffer.from("MARGINFI_GROUP_SEED_STAKED_LIMIT"),
  );
  const solBankKp = Keypair.generate();
  const validators: Validator[] = [];

  const solBank = () => solBankKp.publicKey;
  const wsolOracle = () => oracles.wsolOracle.publicKey;
  const liquidatee = () => users[0];
  const liquidator = () => users[1];
  const stakedGroup = (v: Validator) => [
    v.bank,
    wsolOracle(),
    v.splMint,
    v.splSolPool,
    v.splOnRampPool,
  ];
  /** SOL-tagged collateral filling the account's other slots while measuring; empty when executing. */
  let padGroups: PublicKey[][] = [];
  const liquidateeGroups = (staked: number) => [
    ...validators.slice(0, staked).map(stakedGroup),
    ...padGroups,
    [solBank(), wsolOracle()],
  ];
  /** Fills the account to its 16-balance maximum, each pad bank on its own oracle. */
  const padToFullAccount = (staked: number) => {
    padGroups = Array.from({ length: MAX_BALANCES - 1 - staked }, () => [
      PublicKey.unique(),
      PublicKey.unique(),
    ]);
  };
  /** The liquidator holds the SOL debt bank plus the staked asset it seizes, created mid-liquidation. */
  const liquidatorGroups = () => [
    stakedGroup(validators[0]),
    [solBank(), wsolOracle()],
  ];
  const liquidatorLstAta = () =>
    getAssociatedTokenAddressSync(
      validators[0].splMint,
      liquidator().wallet.publicKey,
    );

  const classicLiquidationIxs = async (staked: number) => {
    const liquidateeAccounts = composeRemainingAccounts(
      liquidateeGroups(staked),
    );
    const liquidatorAccounts = composeRemainingAccounts(liquidatorGroups());
    return [
      ComputeBudgetProgram.setComputeUnitLimit({ units: 1_400_000 }),
      await liquidateIx(liquidator().mrgnBankrunProgram, {
        assetBankKey: validators[0].bank,
        liabilityBankKey: solBank(),
        liquidatorMarginfiAccount: liquidator().accounts.get(ACCOUNT),
        liquidateeMarginfiAccount: liquidatee().accounts.get(ACCOUNT),
        remaining: [
          ...stakedGroup(validators[0]).slice(1),
          wsolOracle(),
          ...liquidatorAccounts,
          ...liquidateeAccounts,
        ],
        amount: SOL(0.1),
        liquidateeAccounts: liquidateeAccounts.length,
        liquidatorAccounts: liquidatorAccounts.length,
      }),
    ];
  };

  const receivershipIxs = async (staked: number) => {
    const program = liquidator().mrgnBankrunProgram;
    const marginfiAccount = liquidatee().accounts.get(ACCOUNT);
    const groups = liquidateeGroups(staked);
    return [
      ComputeBudgetProgram.setComputeUnitLimit({ units: 1_400_000 }),
      await startLiquidationIx(program, {
        marginfiAccount,
        liquidationReceiver: liquidator().wallet.publicKey,
        remaining: composeRemainingAccountsWriteableMeta(groups),
      }),
      await withdrawIx(program, {
        marginfiAccount,
        bank: validators[0].bank,
        tokenAccount: liquidatorLstAta(),
        remaining: stakedGroup(validators[0]),
        amount: SOL(0.05),
      }),
      await repayIx(program, {
        marginfiAccount,
        bank: solBank(),
        tokenAccount: liquidator().wsolAccount,
        amount: SOL(0.05),
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

  it("(admin) inits group, SOL bank, staked settings, 15 validators and their staked banks", async () => {
    await refreshPullOraclesBankrun(oracles, bankrunContext, banksClient);
    const admin = groupAdmin.mrgnBankrunProgram;
    const solConfig = defaultBankConfig();
    solConfig.assetTag = ASSET_TAG_SOL;
    await processBankrunTransaction(
      bankrunContext,
      new Transaction().add(
        await groupInitialize(admin, {
          marginfiGroup: group.publicKey,
          admin: groupAdmin.wallet.publicKey,
        }),
        await addBank(admin, {
          marginfiGroup: group.publicKey,
          feePayer: groupAdmin.wallet.publicKey,
          bankMint: ecosystem.wsolMint.publicKey,
          bank: solBank(),
          config: solConfig,
        }),
      ),
      [groupAdmin.wallet, group, solBankKp],
    );
    await processBankrunTransaction(
      bankrunContext,
      new Transaction().add(
        await configureBankOracle(admin, {
          bank: solBank(),
          type: ORACLE_SETUP_PYTH_PUSH,
          oracle: wsolOracle(),
        }),
        await initStakedSettings(admin, {
          group: group.publicKey,
          feePayer: groupAdmin.wallet.publicKey,
          settings: defaultStakedInterestSettings(wsolOracle()),
        }),
      ),
      [groupAdmin.wallet],
    );

    for (let i = 0; i < STAKED_BANKS; i++) {
      const validator = await createSplStakePoolBankrun(
        await createValidatorBankrun(i),
      );
      await processBankrunTransaction(
        bankrunContext,
        new Transaction().add(
          await addBankPermissionless(admin, {
            marginfiGroup: group.publicKey,
            feePayer: groupAdmin.wallet.publicKey,
            pythOracle: wsolOracle(),
            stakePool: validator.splPool,
            validatorVoteAccount: validator.voteAccount,
            seed: new BN(0),
          }),
        ),
        [groupAdmin.wallet],
      );
      [validator.bank] = deriveBankWithSeed(
        bankrunProgram.programId,
        group.publicKey,
        validator.splMint,
        new BN(0),
      );
      validators.push(validator);
    }

    for (const user of [groupAdmin, liquidatee(), liquidator()]) {
      const account = Keypair.generate();
      await processBankrunTransaction(
        bankrunContext,
        new Transaction().add(
          await accountInit(user.mrgnBankrunProgram, {
            marginfiGroup: group.publicKey,
            marginfiAccount: account.publicKey,
            authority: user.wallet.publicKey,
            feePayer: user.wallet.publicKey,
          }),
        ),
        [user.wallet, account],
      );
      user.accounts.set(ACCOUNT, account.publicKey);
    }

    for (const [user, amount] of [
      [groupAdmin, 50],
      [liquidator(), 10],
    ] as const) {
      await mintToTokenAccount(
        ecosystem.wsolMint.publicKey,
        user.wsolAccount,
        SOL(amount * 2),
      );
      await processBankrunTransaction(
        bankrunContext,
        new Transaction().add(
          await depositIx(user.mrgnBankrunProgram, {
            marginfiAccount: user.accounts.get(ACCOUNT),
            bank: solBank(),
            tokenAccount: user.wsolAccount,
            amount: SOL(amount),
          }),
        ),
        [user.wallet],
      );
    }
    await processBankrunTransaction(
      bankrunContext,
      new Transaction().add(
        createAssociatedTokenAccountIdempotentInstruction(
          liquidator().wallet.publicKey,
          liquidatorLstAta(),
          liquidator().wallet.publicKey,
          validators[0].splMint,
        ),
      ),
      [liquidator().wallet],
    );
  });

  it("(user 0) stakes with every validator and mints its LST", async () => {
    const user = liquidatee();
    const stakeAccounts: PublicKey[] = [];
    for (const validator of validators) {
      const { createTx, stakeAccountKeypair } = createStakeAccount(
        user,
        2 * LAMPORTS_PER_SOL,
      );
      await processBankrunTransaction(bankrunContext, createTx, [
        user.wallet,
        stakeAccountKeypair,
      ]);
      await processBankrunTransaction(
        bankrunContext,
        delegateStake(
          user,
          stakeAccountKeypair.publicKey,
          validator.voteAccount,
        ),
        [user.wallet],
      );
      stakeAccounts.push(stakeAccountKeypair.publicKey);
    }

    // Stake activates at the epoch boundary; the stake program is frozen for the first slots after.
    // This moves the shared bankrun clock for every later spec, and leaves these pools exactly one
    // epoch from `StakePoolStale`.
    const { epoch, slot } = await getEpochAndSlot(banksClient);
    bankrunContext.warpToEpoch(BigInt(epoch + 1));
    const { slot: slotAfterWarp } = await getEpochAndSlot(banksClient);
    bankrunContext.warpToSlot(BigInt(Math.max(slot, slotAfterWarp) + 3));
    await refreshPullOraclesBankrun(oracles, bankrunContext, banksClient);

    for (let i = 0; i < validators.length; i++) {
      const ixes = await depositToSinglePoolIxes(
        bankRunProvider.connection,
        user.wallet.publicKey,
        validators[i].splPool,
        stakeAccounts[i],
      );
      await processBankrunTransaction(
        bankrunContext,
        new Transaction().add(...ixes),
        [user.wallet],
      );
    }
  });

  it("reports how many positions each liquidation path covers on a full account", async () => {
    const payer = liquidator().wallet.publicKey;
    const fits = { classic: 0, receivership: 0 };
    console.log(
      `\nstaked + SOL, ${MAX_BALANCES} balances, limit ${TX_ACCOUNT_LOCK_LIMIT} locks:`,
    );
    for (let staked = 1; staked <= STAKED_BANKS; staked++) {
      padToFullAccount(staked);
      const classic = countTxAccountLocks(
        payer,
        await classicLiquidationIxs(staked),
      );
      const receivership = countTxAccountLocks(
        payer,
        await receivershipIxs(staked),
      );
      console.log(
        `  ${String(staked).padStart(2)} + ${
          MAX_BALANCES - staked
        }: classic ${String(classic).padStart(
          2,
        )}, receivership ${receivership}`,
      );
      if (fits.classic === staked - 1 && classic <= TX_ACCOUNT_LOCK_LIMIT)
        fits.classic = staked;
      if (
        fits.receivership === staked - 1 &&
        receivership <= TX_ACCOUNT_LOCK_LIMIT
      )
        fits.receivership = staked;
    }
    padGroups = [];
    console.log(
      `  covers: classic ${fits.classic}, receivership ${fits.receivership} (cap ${CAP})`,
    );
    if (fits.receivership < CAP)
      console.log(
        `KNOWN GAP: staked receivership covers only ${fits.receivership} of the ${CAP} positions the cap allows`,
      );
    assert.isAtLeast(fits.classic, CAP, "classic no longer covers the cap");
    assert.isAtLeast(
      fits.receivership,
      CAP,
      "receivership no longer covers the cap",
    );
  });

  it("(user 0) fills the account to the staked cap and borrows SOL", async () => {
    const user = liquidatee();
    const marginfiAccount = user.accounts.get(ACCOUNT);
    for (const validator of validators.slice(0, CAP)) {
      await processBankrunTransaction(
        bankrunContext,
        new Transaction().add(
          await depositIx(user.mrgnBankrunProgram, {
            marginfiAccount,
            bank: validator.bank,
            tokenAccount: getAssociatedTokenAddressSync(
              validator.splMint,
              user.wallet.publicKey,
            ),
            amount: SOL(0.5),
          }),
        ),
        [user.wallet],
      );
    }
    await sendV0(
      [
        ComputeBudgetProgram.setComputeUnitLimit({ units: 1_400_000 }),
        await borrowIx(user.mrgnBankrunProgram, {
          marginfiAccount,
          bank: solBank(),
          tokenAccount: user.wsolAccount,
          remaining: composeRemainingAccounts(liquidateeGroups(CAP)),
          amount: SOL(0.5),
        }),
      ],
      user.wallet,
    );
  });

  it("(admin) makes user 0 unhealthy by inflating the SOL liability weight", async () => {
    const config = blankBankConfigOptRaw();
    config.liabilityWeightInit = bigNumberToWrappedI80F48(210);
    config.liabilityWeightMaint = bigNumberToWrappedI80F48(200);
    await processBankrunTransaction(
      bankrunContext,
      new Transaction().add(
        await configureBank(groupAdmin.mrgnBankrunProgram, {
          bank: solBank(),
          bankConfigOpt: config,
        }),
      ),
      [groupAdmin.wallet],
    );
  });

  it("(user 1) liquidates user 0 with the classic instruction", async () => {
    await sendV0(await classicLiquidationIxs(CAP), liquidator().wallet);
  });

  it("(user 1) liquidates user 0 with the receivership sandwich", async () => {
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
    await sendV0(await receivershipIxs(CAP), liquidator().wallet);
  });
});
