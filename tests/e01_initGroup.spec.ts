import { BN } from "@coral-xyz/anchor";
import { AccountMeta, PublicKey, Transaction } from "@solana/web3.js";
import {
  addBankWithSeed,
  groupConfigure,
  groupInitialize,
} from "./utils/group-instructions";
import {
  bankrunContext,
  bankrunProgram,
  banksClient,
  ecosystem,
  EMODE_SEED,
  emodeAdmin,
  emodeGroup,
  groupAdmin,
  oracles,
  verbose,
} from "./rootHooks";
import { assertKeyDefault, assertKeysEqual } from "./utils/genericTests";
import {
  ASSET_TAG_SOL,
  defaultBankConfig,
  I80F48_ZERO,
  ORACLE_SETUP_PYTH_PUSH,
} from "./utils/types";
import { deriveBankWithSeed } from "./utils/pdas";
import { bigNumberToWrappedI80F48 } from "@mrgnlabs/mrgn-common";
import { getBankrunBlockhash } from "./utils/tools";

describe("Init e-mode enabled group and banks", () => {
  const seed = new BN(EMODE_SEED);

  it("(admin) Init group - happy path", async () => {
    let tx = new Transaction().add(
      await groupInitialize(groupAdmin.mrgnBankrunProgram, {
        marginfiGroup: emodeGroup.publicKey,
        admin: groupAdmin.wallet.publicKey,
      })
    );
    tx.recentBlockhash = await getBankrunBlockhash(bankrunContext);
    tx.sign(groupAdmin.wallet, emodeGroup);
    await banksClient.processTransaction(tx);

    let group = await bankrunProgram.account.marginfiGroup.fetch(
      emodeGroup.publicKey
    );
    assertKeysEqual(group.admin, groupAdmin.wallet.publicKey);
    assertKeyDefault(group.emodeAdmin);
    if (verbose) {
      console.log("*init group: " + emodeGroup.publicKey);
      console.log(" group admin: " + group.admin);
    }
  });

  it("(admin) Set the emode admin - happy path", async () => {
    let tx = new Transaction().add(
      await groupConfigure(groupAdmin.mrgnBankrunProgram, {
        marginfiGroup: emodeGroup.publicKey,
        newEmodeAdmin: emodeAdmin.wallet.publicKey,
      })
    );
    tx.recentBlockhash = await getBankrunBlockhash(bankrunContext);
    tx.sign(groupAdmin.wallet);
    await banksClient.processTransaction(tx);

    let group = await bankrunProgram.account.marginfiGroup.fetch(
      emodeGroup.publicKey
    );
    assertKeysEqual(group.emodeAdmin, emodeAdmin.wallet.publicKey);
  });

  it("(admin) Add bank (USDC)", async () => {
    await addBankTest({
      bankMint: ecosystem.usdcMint.publicKey,
      oracleKey: oracles.usdcOracle.publicKey,
      oracleMeta: {
        pubkey: oracles.usdcOracle.publicKey,
        isSigner: false,
        isWritable: false,
      },
      seed: seed,
      verboseMessage: "*init USDC bank:",
    });
  });

  it("(admin) Add bank (also a stablecoin)", async () => {
    await addBankTest({
      bankMint: ecosystem.usdcMint.publicKey,
      oracleKey: oracles.usdcOracle.publicKey,
      oracleMeta: {
        pubkey: oracles.usdcOracle.publicKey,
        isSigner: false,
        isWritable: false,
      },
      seed: seed.addn(1),
      verboseMessage: "*init USDC bank:",
    });
  });

  it("(admin) Add bank (SOL)", async () => {
    await addBankTest({
      assetTag: ASSET_TAG_SOL,
      bankMint: ecosystem.wsolMint.publicKey,
      oracleKey: oracles.wsolOracle.publicKey,
      oracleMeta: {
        pubkey: oracles.wsolOracle.publicKey,
        isSigner: false,
        isWritable: false,
      },
      seed: seed,
      verboseMessage: "*init SOL bank:",
    });
  });

  it("(admin) Add bank (LST)", async () => {
    await addBankTest({
      bankMint: ecosystem.lstAlphaMint.publicKey,
      oracleKey: oracles.pythPullLst.publicKey,
      oracleMeta: {
        pubkey: oracles.pythPullLst.publicKey, // NOTE: Price V2 update
        isSigner: false,
        isWritable: false,
      },
      seed: seed,
      verboseMessage: "*init LST A bank:",
    });
  });

  it("(admin) Add another bank (also an LST)", async () => {
    await addBankTest({
      bankMint: ecosystem.lstAlphaMint.publicKey,
      oracleKey: oracles.pythPullLst.publicKey,
      oracleMeta: {
        pubkey: oracles.pythPullLst.publicKey, // NOTE: Price V2 update
        isSigner: false,
        isWritable: false,
      },
      seed: seed.addn(1),
      verboseMessage: "*init LST B bank:",
    });
  });

  async function addBankTest(options: {
    assetTag?: number;
    bankMint: PublicKey;
    oracleKey: PublicKey;
    oracleMeta: AccountMeta;
    // Function to adjust the seed (for example, seed.addn(1))
    seed: BN;
    verboseMessage: string;
  }) {
    const { assetTag, bankMint, oracleKey, oracleMeta, seed, verboseMessage } =
      options;

    // Set configuration; override assetTag if provided
    const config = defaultBankConfig();
    // Use a reduced weight for this test suite to see the impact of emode.
    config.assetWeightInit = bigNumberToWrappedI80F48(0.5);
    config.assetWeightMaint = bigNumberToWrappedI80F48(0.6);
    // The default limit is somewhat small for SOL/LST with 9 decimals, so we bump it here.
    config.depositLimit = new BN(100_000_000_000_000);
    config.borrowLimit = new BN(100_000_000_000_000);
    // We don't want origination fees messing with debt here
    config.interestRateConfig.protocolOriginationFee = I80F48_ZERO;
    if (assetTag) {
      config.assetTag = assetTag;
    }

    // Calculate bank key using the (optionally modified) seed
    const [bankKey] = deriveBankWithSeed(
      bankrunProgram.programId,
      emodeGroup.publicKey,
      bankMint,
      seed
    );

    const setupType = ORACLE_SETUP_PYTH_PUSH;
    const config_ix = await groupAdmin.mrgnProgram.methods
      .lendingPoolConfigureBankOracle(setupType, oracleKey)
      .accountsPartial({
        group: emodeGroup.publicKey,
        bank: bankKey,
        admin: groupAdmin.wallet.publicKey,
      })
      .remainingAccounts([oracleMeta])
      .instruction();

    const addBankIx = await addBankWithSeed(groupAdmin.mrgnBankrunProgram, {
      marginfiGroup: emodeGroup.publicKey,
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
});
