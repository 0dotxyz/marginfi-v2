import { Program } from "@coral-xyz/anchor";
import { createMintToInstruction } from "@solana/spl-token";
import {
  PublicKey,
  Keypair,
  Transaction,
  AccountMeta,
  ComputeBudgetProgram,
} from "@solana/web3.js";
import BN from "bn.js";
import {
  groupAdmin,
  bankrunContext,
  banksClient,
  verbose,
  ecosystem,
  oracles,
  bankrunProgram,
  users,
  kaminoAccounts,
  MARKET,
  TOKEN_A_RESERVE,
  A_FARM_STATE,
  farmAccounts,
  FARMS_PROGRAM_ID,
} from "./rootHooks";
import { addBankWithSeed, groupInitialize } from "./utils/group-instructions";
import {
  deriveBankWithSeed,
  deriveBaseObligation,
  deriveLiquidityVaultAuthority,
} from "./utils/pdas";
import { getBankrunBlockhash } from "./utils/spl-staking-utils";
import { accountInit } from "./utils/user-instructions";
import { bigNumberToWrappedI80F48 } from "@mrgnlabs/mrgn-common";
import {
  defaultBankConfig,
  I80F48_ZERO,
  makeRatePoints,
  ORACLE_SETUP_PYTH_PUSH,
} from "./utils/types";
import {
  defaultKaminoBankConfig,
} from "./utils/kamino-utils";
import {
  makeAddKaminoBankIx,
  makeInitObligationIx,
} from "./utils/kamino-instructions";
import { processBankrunTransaction } from "./utils/tools";

/**
 * Initialize a group, create N banks, fund everyone, and init accounts.
 *
 * @param numberOfBanks - how many banks to add (≤ MAX_BALANCES)
 * @returns the globals you’ll want in your tests (list of banks, group keypair)
 */
export const genericMultiBankTestSetup = async (
  numberOfBanks: number,
  userAccountName: string,
  groupSeed: Buffer,
  startingSeed: number
): Promise<{
  banks: PublicKey[];
  throwawayGroup: Keypair;
}> => {
  const USER_ACCOUNT_THROWAWAY = userAccountName;

  let banks: PublicKey[] = [];
  const throwawayGroup = Keypair.fromSeed(groupSeed);

  // 1) init the group
  {
    const tx = new Transaction();
    tx.add(
      await groupInitialize(groupAdmin.mrgnBankrunProgram, {
        marginfiGroup: throwawayGroup.publicKey,
        admin: groupAdmin.wallet.publicKey,
      })
    );
    tx.recentBlockhash = await getBankrunBlockhash(bankrunContext);
    tx.sign(groupAdmin.wallet, throwawayGroup);
    await banksClient.processTransaction(tx);

    if (verbose) console.log(`*init group: ${throwawayGroup.publicKey}`);
  }

  // 2) add banks
  {
    // Process banks sequentially to avoid "Account in use" error
    for (let i = 0; i < numberOfBanks; i++) {
      const seed = startingSeed + i;
      await addGenericBank(throwawayGroup, {
        bankMint: ecosystem.lstAlphaMint.publicKey,
        oracle: oracles.pythPullLst.publicKey,
        oracleMeta: {
          pubkey: oracles.pythPullLst.publicKey,
          isSigner: false,
          isWritable: false,
        },
        seed: new BN(seed),
        verboseMessage: verbose ? `*init LST #${seed}:` : undefined,
      });

      const [bankPk] = deriveBankWithSeed(
        bankrunProgram.programId,
        throwawayGroup.publicKey,
        ecosystem.lstAlphaMint.publicKey,
        new BN(seed)
      );
      banks.push(bankPk);
    }
  }

  // 3) fund users + admin
  {
    // Use bankrun payer as mint authority (it created the mints in rootHooks.ts)
    const payer = bankrunContext.payer;

    for (const u of users) {
      const tx = new Transaction();
      tx.add(
        createMintToInstruction(
          ecosystem.lstAlphaMint.publicKey,
          u.lstAlphaAccount,
          payer.publicKey,
          10_000 * 10 ** ecosystem.lstAlphaDecimals
        )
      );
      tx.recentBlockhash = await getBankrunBlockhash(bankrunContext);
      tx.sign(payer);
      await banksClient.processTransaction(tx);
    }

    const txAdmin = new Transaction();
    txAdmin.add(
      createMintToInstruction(
        ecosystem.lstAlphaMint.publicKey,
        groupAdmin.lstAlphaAccount,
        payer.publicKey,
        10_000 * 10 ** ecosystem.lstAlphaDecimals
      )
    );
    txAdmin.recentBlockhash = await getBankrunBlockhash(bankrunContext);
    txAdmin.sign(payer);
    await banksClient.processTransaction(txAdmin);
  }

  // 4) init user accounts (and admin) if not already init
  {
    for (const [i, u] of users.entries()) {
      if (u.accounts.has(USER_ACCOUNT_THROWAWAY)) {
        if (verbose) console.log(`Skipped creating user account #${i}`);
      } else {
        const kp = Keypair.generate();
        u.accounts.set(USER_ACCOUNT_THROWAWAY, kp.publicKey);
        if (verbose) console.log(`Initialized user #${i}: ${kp.publicKey}`);

        const tx = new Transaction();
        tx.add(
          await accountInit(u.mrgnBankrunProgram, {
            marginfiGroup: throwawayGroup.publicKey,
            marginfiAccount: kp.publicKey,
            authority: u.wallet.publicKey,
            feePayer: u.wallet.publicKey,
          })
        );
        tx.recentBlockhash = await getBankrunBlockhash(bankrunContext);
        tx.sign(u.wallet, kp);
        await banksClient.processTransaction(tx);
      }
    }

    if (!groupAdmin.accounts.has(USER_ACCOUNT_THROWAWAY)) {
      const adminKp = Keypair.generate();
      groupAdmin.accounts.set(USER_ACCOUNT_THROWAWAY, adminKp.publicKey);
      if (verbose)
        console.log(`Initialized admin account: ${adminKp.publicKey}`);

      const tx = new Transaction();
      tx.add(
        await accountInit(groupAdmin.mrgnBankrunProgram, {
          marginfiGroup: throwawayGroup.publicKey,
          marginfiAccount: adminKp.publicKey,
          authority: groupAdmin.wallet.publicKey,
          feePayer: groupAdmin.wallet.publicKey,
        })
      );
      tx.recentBlockhash = await getBankrunBlockhash(bankrunContext);
      tx.sign(groupAdmin.wallet, adminKp);
      await banksClient.processTransaction(tx);
    } else if (verbose) {
      console.log("Skipped creating admin account");
    }
  }

  return { banks, throwawayGroup };
};

/**
 * Sets up a generic bank for the provided group.
 * * deposit/borrow limit = 100_000_000_000_000
 * * protocolOriginationFee = 0
 * * all fees = 0
 * * Plataeu rate = 20% (one point .8, .2)
 * * Optimal rate = 80% (one point .8, .2)
 * * asset tag over-rides if provided.
 * * otherwise uses `defaultBankConfig()`
 * @param throwawayGroup
 * @param options
 */
async function addGenericBank(
  throwawayGroup: Keypair,
  options: {
    assetTag?: number;
    bankMint: PublicKey;
    oracle: PublicKey;
    oracleMeta: AccountMeta;
    // Function to adjust the seed (for example, seed.addn(1))
    seed: BN;
    verboseMessage: string;
  }
) {
  const { assetTag, bankMint, oracle, oracleMeta, seed, verboseMessage } =
    options;

  const config = defaultBankConfig();
  config.assetWeightInit = bigNumberToWrappedI80F48(0.5);
  config.assetWeightMaint = bigNumberToWrappedI80F48(0.6);

  // The default limit is somewhat small for SOL/LST with 9 decimals, so we bump it here.
  config.depositLimit = new BN(100_000_000_000_000);
  config.borrowLimit = new BN(100_000_000_000_000);
  // We don't want origination fees messing with debt here
  config.interestRateConfig.protocolOriginationFee = I80F48_ZERO;
  config.interestRateConfig.points = makeRatePoints([0.8], [0.2]);
  if (assetTag) {
    config.assetTag = assetTag;
  }

  // Calculate bank key using the (optionally modified) seed
  const [bankKey] = deriveBankWithSeed(
    bankrunProgram.programId,
    throwawayGroup.publicKey,
    bankMint,
    seed
  );

  const setupType = ORACLE_SETUP_PYTH_PUSH;
  const config_ix = await groupAdmin.mrgnProgram.methods
    .lendingPoolConfigureBankOracle(setupType, oracle)
    .accountsPartial({
      group: throwawayGroup.publicKey,
      bank: bankKey,
      admin: groupAdmin.wallet.publicKey,
    })
    .remainingAccounts([oracleMeta])
    .instruction();

  const addBankIx = await addBankWithSeed(groupAdmin.mrgnBankrunProgram, {
    marginfiGroup: throwawayGroup.publicKey,
    feePayer: groupAdmin.wallet.publicKey,
    bankMint: bankMint,
    config: config,
    seed,
  });

  const tx = new Transaction();
  tx.add(addBankIx, config_ix);
  tx.recentBlockhash = await getBankrunBlockhash(bankrunContext);
  tx.sign(groupAdmin.wallet);
  await banksClient.processTransaction(tx);

  if (verbose) {
    console.log(`${verboseMessage} ${bankKey}`);
  }
}

/**
 * Initialize a group, create N banks, fund everyone, and init accounts.
 *
 * @param numberOfBanks - how many banks to add (≤ MAX_BALANCES)
 * @returns the globals you’ll want in your tests (list of banks, group keypair)
 */
export const genericKaminoMultiBankTestSetup = async (
  numberOfBanks: number,
  userAccountName: string,
  groupSeed: Buffer,
  startingSeed: number
): Promise<{
  kaminoBanks: PublicKey[];
  regularBanks: PublicKey[];
  throwawayGroup: Keypair;
}> => {
  const USER_ACCOUNT_THROWAWAY = userAccountName;
  const market = kaminoAccounts.get(MARKET);
  const tokenAReserve = kaminoAccounts.get(TOKEN_A_RESERVE);
  const farmState = farmAccounts.get(A_FARM_STATE);

  let kaminoBanks: PublicKey[] = [];
  let regularBanks: PublicKey[] = [];
  const throwawayGroup = Keypair.fromSeed(groupSeed);

  // 1) init the group
  {
    const tx = new Transaction();
    tx.add(
      await groupInitialize(groupAdmin.mrgnBankrunProgram, {
        marginfiGroup: throwawayGroup.publicKey,
        admin: groupAdmin.wallet.publicKey,
      })
    );
    tx.recentBlockhash = await getBankrunBlockhash(bankrunContext);
    tx.sign(groupAdmin.wallet, throwawayGroup);
    await banksClient.processTransaction(tx);

    if (verbose) console.log(`*init group: ${throwawayGroup.publicKey}`);
  }

  // 2) add kamino banks (all use the same Kamino reserve for simplicity.)
  {
    // Process banks sequentially to avoid "Account in use" error
    for (let i = 0; i < numberOfBanks; i++) {
      const seed = startingSeed + i;

      // Execute addGenericKaminoBank sequentially
      await addGenericKaminoBank(
        throwawayGroup,
        market,
        tokenAReserve,
        ecosystem.tokenAMint.publicKey,
        oracles.tokenAOracle.publicKey,
        oracles.tokenAOracleFeed.publicKey,
        new BN(seed),
        verbose ? `*init Token A #${seed}:` : undefined,
        farmState ? farmState : null
      );

      const [bankPk] = deriveBankWithSeed(
        bankrunProgram.programId,
        throwawayGroup.publicKey,
        ecosystem.tokenAMint.publicKey,
        new BN(seed)
      );
      kaminoBanks.push(bankPk);
    }
  }

  // 2.5 Init regular banks
  {
    // Process banks sequentially to avoid "Account in use" error
    for (let i = 0; i < numberOfBanks; i++) {
      const seed = startingSeed + i;

      // Execute addGenericBank sequentially
      await addGenericBank(throwawayGroup, {
        bankMint: ecosystem.lstAlphaMint.publicKey,
        oracle: oracles.pythPullLst.publicKey,
        oracleMeta: {
          pubkey: oracles.pythPullLst.publicKey,
          isSigner: false,
          isWritable: false,
        },
        seed: new BN(seed),
        verboseMessage: verbose ? `*init LST #${seed}:` : undefined,
      });

      const [bankPk] = deriveBankWithSeed(
        bankrunProgram.programId,
        throwawayGroup.publicKey,
        ecosystem.lstAlphaMint.publicKey,
        new BN(seed)
      );
      regularBanks.push(bankPk);
    }
  }

  // 3) fund users + admin
  {
    // Use bankrun payer as mint authority (it created the mints in rootHooks.ts)
    const payer = bankrunContext.payer;

    for (const u of users) {
      const tx = new Transaction();
      tx.add(
        createMintToInstruction(
          ecosystem.lstAlphaMint.publicKey,
          u.lstAlphaAccount,
          payer.publicKey,
          10_000 * 10 ** ecosystem.lstAlphaDecimals
        )
      );
      tx.recentBlockhash = await getBankrunBlockhash(bankrunContext);
      tx.sign(payer);
      await banksClient.processTransaction(tx);
    }

    const txAdmin = new Transaction();
    txAdmin.add(
      createMintToInstruction(
        ecosystem.lstAlphaMint.publicKey,
        groupAdmin.lstAlphaAccount,
        payer.publicKey,
        10_000 * 10 ** ecosystem.lstAlphaDecimals
      )
    );
    txAdmin.recentBlockhash = await getBankrunBlockhash(bankrunContext);
    txAdmin.sign(payer);
    await banksClient.processTransaction(txAdmin);
  }

  // 4) init user accounts (and admin) if not already init
  {
    for (const [i, u] of users.entries()) {
      if (u.accounts.has(USER_ACCOUNT_THROWAWAY)) {
        if (verbose) console.log(`Skipped creating user account #${i}`);
      } else {
        const kp = Keypair.generate();
        u.accounts.set(USER_ACCOUNT_THROWAWAY, kp.publicKey);
        if (verbose) console.log(`Initialized user #${i}: ${kp.publicKey}`);

        const tx = new Transaction();
        tx.add(
          await accountInit(u.mrgnBankrunProgram, {
            marginfiGroup: throwawayGroup.publicKey,
            marginfiAccount: kp.publicKey,
            authority: u.wallet.publicKey,
            feePayer: u.wallet.publicKey,
          })
        );
        tx.recentBlockhash = await getBankrunBlockhash(bankrunContext);
        tx.sign(u.wallet, kp);
        await banksClient.processTransaction(tx);
      }
    }

    if (!groupAdmin.accounts.has(USER_ACCOUNT_THROWAWAY)) {
      const adminKp = Keypair.generate();
      groupAdmin.accounts.set(USER_ACCOUNT_THROWAWAY, adminKp.publicKey);
      if (verbose)
        console.log(`Initialized admin account: ${adminKp.publicKey}`);

      const tx = new Transaction();
      tx.add(
        await accountInit(groupAdmin.mrgnBankrunProgram, {
          marginfiGroup: throwawayGroup.publicKey,
          marginfiAccount: adminKp.publicKey,
          authority: groupAdmin.wallet.publicKey,
          feePayer: groupAdmin.wallet.publicKey,
        })
      );
      tx.recentBlockhash = await getBankrunBlockhash(bankrunContext);
      tx.sign(groupAdmin.wallet, adminKp);
      await banksClient.processTransaction(tx);
    } else if (verbose) {
      console.log("Skipped creating admin account");
    }
  }

  return {
    kaminoBanks,
    regularBanks,
    throwawayGroup,
  };
};

/**
 *
 * @param throwawayGroup
 * @param market
 * @param reserve
 * @param mint
 * @param oracle
 * @param oracleFeedId
 * @param seed
 * @param verboseMessage
 * @param farmState - required if reserve has a farm enabled
 */
async function addGenericKaminoBank(
  throwawayGroup: Keypair,
  market: PublicKey,
  reserve: PublicKey,
  mint: PublicKey,
  oracle: PublicKey,
  oracleFeedId: PublicKey,
  seed: BN,
  verboseMessage: string,
  farmState: PublicKey | null
) {
  const config = defaultKaminoBankConfig(oracle);
  const [bankKey] = deriveBankWithSeed(
    bankrunProgram.programId,
    throwawayGroup.publicKey,
    mint,
    seed
  );
  let initBankTx = new Transaction().add(
    await makeAddKaminoBankIx(
      groupAdmin.mrgnBankrunProgram,
      {
        group: throwawayGroup.publicKey,
        feePayer: groupAdmin.wallet.publicKey,
        bankMint: mint,
        integrationAcc1: reserve,
        kaminoMarket: market,
        oracle: oracle,
      },
      { config: config, seed }
    )
  );

  const [lendingVaultAuthority] = deriveLiquidityVaultAuthority(
    bankrunProgram.programId,
    bankKey
  );
  const [obligation] = deriveBaseObligation(lendingVaultAuthority, market);
  const [userState] = PublicKey.findProgramAddressSync(
    [Buffer.from("user"), farmState.toBuffer(), obligation.toBuffer()],
    FARMS_PROGRAM_ID
  );
  // console.log("farm state passed: " + farmState + " user " + userState);

  await processBankrunTransaction(bankrunContext, initBankTx, [
    groupAdmin.wallet,
  ]);
  let initObligationTx = new Transaction().add(
    ComputeBudgetProgram.setComputeUnitLimit({ units: 2_000_000 }),
    await makeInitObligationIx(
      groupAdmin.mrgnBankrunProgram,
      {
        feePayer: groupAdmin.wallet.publicKey,
        bank: bankKey,
        signerTokenAccount: groupAdmin.tokenAAccount,
        lendingMarket: market,
        reserveLiquidityMint: ecosystem.tokenAMint.publicKey,
        pythOracle: oracles.tokenAOracle.publicKey,
        reserveFarmState: farmState,
        obligationFarmUserState: userState,
      },
      new BN(100)
    )
  );
  await processBankrunTransaction(bankrunContext, initObligationTx, [
    groupAdmin.wallet,
  ]);

  if (verbose) {
    console.log(`${verboseMessage} ${bankKey}`);
  }
}
