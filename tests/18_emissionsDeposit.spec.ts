import { BN, Program } from "@coral-xyz/anchor";
import {
  Keypair,
  PublicKey,
  SystemProgram,
  Transaction,
} from "@solana/web3.js";
import {
  createMintToInstruction,
  createInitializeMint2Instruction,
  createAssociatedTokenAccountInstruction,
  getAssociatedTokenAddressSync,
  getMintLen,
  ExtensionType,
  createInitializeTransferFeeConfigInstruction,
  createInitializeTransferHookInstruction,
  TOKEN_PROGRAM_ID,
  TOKEN_2022_PROGRAM_ID,
} from "@solana/spl-token";
import { Marginfi } from "../target/types/marginfi";
import {
  bankKeypairA,
  bankrunContext,
  bankrunProgram,
  bankRunProvider,
  ecosystem,
  globalProgramAdmin,
  groupAdmin,
  marginfiGroup,
} from "./rootHooks";
import { assert } from "chai";
import { getTokenBalance, expectFailedTxWithError } from "./utils/genericTests";
import { deriveLiquidityVault } from "./utils/pdas";
import {
  configureBank,
  configureBankOracle,
  lendingPoolEmissionsDeposit,
  panicPause,
  panicUnpause,
  propagateFeeState,
} from "./utils/group-instructions";
import { setEmissionsDirect, processBankrunTransaction } from "./utils/tools";
import {
  blankBankConfigOptRaw,
  defaultBankConfig,
  ORACLE_CONF_INTERVAL,
  ORACLE_SETUP_PYTH_PUSH,
} from "./utils/types";
import { wrappedI80F48toBigNumber } from "@mrgnlabs/mrgn-common";
import { BankrunProvider } from "anchor-bankrun";
import { createMintBankrun } from "./utils/mocks";
import {
  createBankrunPythFeedAccount,
  createBankrunPythOracleAccount,
  setPythPullOraclePrice,
  PYTH_RECEIVER_PROGRAM_ID,
} from "./utils/bankrun-oracles";
import { accountInit } from "./utils/user-instructions";

function assertSameBankDeposit(
  sharesBefore: { value: number[] },
  sharesAfter: { value: number[] },
  shareValueBefore: { value: number[] },
  shareValueAfter: { value: number[] },
  liquidityVaultBefore: number,
  liquidityVaultAfter: number,
  emissionsDepositAmount: number,
) {
  const beforeVal = wrappedI80F48toBigNumber(shareValueBefore).toNumber();
  const totalDeposited =
    wrappedI80F48toBigNumber(sharesBefore).toNumber() * beforeVal;
  assert.equal(
    wrappedI80F48toBigNumber(sharesAfter).toString(),
    wrappedI80F48toBigNumber(sharesBefore).toString(),
    "total asset shares should be unchanged",
  );
  // Should be roughly equal. If at all interest accrual happens, time barely passes between
  // this and the last one.
  assert.approximately(
    wrappedI80F48toBigNumber(shareValueAfter).toNumber(),
    beforeVal * (1 + emissionsDepositAmount / totalDeposited),
    beforeVal * 10 ** -10,
  );
  assert.equal(
    liquidityVaultAfter - liquidityVaultBefore,
    emissionsDepositAmount,
  );
}

describe("Same-bank deposit", () => {
  let program: Program<Marginfi>;
  let provider: BankrunProvider;
  let depositor: PublicKey;
  let emissionsMint: PublicKey;
  let operationalState;

  before(async () => {
    program = bankrunProgram;
    provider = bankRunProvider;
    depositor = bankrunContext.payer.publicKey;

    const bank = await program.account.bank.fetch(bankKeypairA.publicKey);
    operationalState = bank.config.operationalState; // Expects that the operational state != killedByBankruptcy.
    emissionsMint = await setEmissionsDirect(
      provider,
      bankKeypairA.publicKey,
      bank.mint,
    );
  });

  after(async () => {
    await setBankState(operationalState);
    await setEmissionsDirect(provider, bankKeypairA.publicKey, emissionsMint);
  });

  it("deposit same-mint emissions updates share value", async () => {
    // Mint 50 Token A to ATA owned by the bankrun payer
    const depositorAmount = 50;
    const fundingAta = getAssociatedTokenAddressSync(
      ecosystem.tokenAMint.publicKey,
      depositor,
    );

    let fundTx = new Transaction();
    fundTx.add(
      createMintToInstruction(
        ecosystem.tokenAMint.publicKey,
        fundingAta,
        depositor,
        BigInt(depositorAmount * 10 ** ecosystem.tokenADecimals),
      ),
    );
    await provider.sendAndConfirm(fundTx);

    // Snapshot bank and liquidity vault
    const bankBefore = await program.account.bank.fetch(bankKeypairA.publicKey);
    const [sharesBefore, shareValueBefore] = [
      bankBefore.totalAssetShares,
      bankBefore.assetShareValue,
    ];
    const [liquidityVault] = deriveLiquidityVault(
      program.programId,
      bankKeypairA.publicKey,
    );
    const liquidityVaultBefore = await getTokenBalance(
      provider,
      liquidityVault,
    );

    // Emissions deposit of 50 Token A from bankrun payer into liquidity vault
    const emissionsDepositAmount =
      depositorAmount * 10 ** ecosystem.tokenADecimals;
    const ix = await lendingPoolEmissionsDeposit(program, {
      bank: bankKeypairA.publicKey,
      mint: bankBefore.mint,
      fundingAccount: fundingAta,
      depositor: depositor,
      liquidityVault: liquidityVault,
      amount: new BN(emissionsDepositAmount),
    });
    let tx = new Transaction().add(ix);
    await provider.sendAndConfirm(tx);

    // Fetch after state
    const bankAfter = await program.account.bank.fetch(bankKeypairA.publicKey);
    const [sharesAfter, shareValueAfter] = [
      bankAfter.totalAssetShares,
      bankAfter.assetShareValue,
    ];

    const liquidityVaultAfter = await getTokenBalance(provider, liquidityVault);

    assertSameBankDeposit(
      sharesBefore,
      sharesAfter,
      shareValueBefore,
      shareValueAfter,
      liquidityVaultBefore,
      liquidityVaultAfter,
      emissionsDepositAmount,
    );
  });

  const setBankState = async (
    state:
      | { paused: undefined }
      | { operational: undefined }
      | { reduceOnly: undefined },
  ) => {
    const cfg = blankBankConfigOptRaw();
    cfg.operationalState = state;
    await groupAdmin.mrgnProgram.provider.sendAndConfirm(
      new Transaction().add(
        await configureBank(groupAdmin.mrgnProgram, {
          bank: bankKeypairA.publicKey,
          bankConfigOpt: cfg,
        }),
      ),
    );
  };

  const setProtocolPaused = async (paused: boolean) => {
    const controlIx = paused
      ? await panicPause(globalProgramAdmin.mrgnProgram, {})
      : await panicUnpause(globalProgramAdmin.mrgnProgram, {});
    const tx = new Transaction().add(
      controlIx,
      await propagateFeeState(globalProgramAdmin.mrgnProgram, {
        group: marginfiGroup.publicKey,
      }),
    );
    await globalProgramAdmin.mrgnProgram.provider.sendAndConfirm(tx);
  };

  it("emissions deposit fails when bank is paused", async () => {
    await setBankState({ paused: undefined });
    const bank = await program.account.bank.fetch(bankKeypairA.publicKey);

    const ix = await lendingPoolEmissionsDeposit(program, {
      bank: bankKeypairA.publicKey,
      mint: bank.mint,
      fundingAccount: getAssociatedTokenAddressSync(
        ecosystem.tokenAMint.publicKey,
        depositor,
      ),
      depositor,
      liquidityVault: bank.liquidityVault,
      amount: new BN(1),
    });

    await expectFailedTxWithError(
      async () => {
        await provider.sendAndConfirm(new Transaction().add(ix));
      },
      "BankPaused",
      6016,
    );
  });

  it("emissions deposit fails when bank is reduce-only", async () => {
    await setBankState({ reduceOnly: undefined });
    const bank = await program.account.bank.fetch(bankKeypairA.publicKey);

    const ix = await lendingPoolEmissionsDeposit(program, {
      bank: bankKeypairA.publicKey,
      mint: bank.mint,
      fundingAccount: getAssociatedTokenAddressSync(
        ecosystem.tokenAMint.publicKey,
        depositor,
      ),
      depositor,
      liquidityVault: bank.liquidityVault,
      amount: new BN(1),
    });
    await expectFailedTxWithError(
      async () => {
        await provider.sendAndConfirm(new Transaction().add(ix));
      },
      "BankReduceOnly",
      6017,
    );
  });

  it("emissions deposit with mismatched mint fails", async () => {
    const emissionsMint = Keypair.generate();
    await createMintBankrun(
      provider.context,
      provider.wallet.payer,
      9,
      emissionsMint,
    );
    await setEmissionsDirect(
      provider,
      bankKeypairA.publicKey,
      emissionsMint.publicKey,
    );

    await setBankState({ operational: undefined });
    const bank = await program.account.bank.fetch(bankKeypairA.publicKey);

    const ix = await program.methods
      .lendingPoolEmissionsDeposit(new BN(1))
      .accountsStrict({
        group: marginfiGroup.publicKey,
        bank: bankKeypairA.publicKey,
        mint: bank.emissionsMint,
        emissionsFundingAccount: getAssociatedTokenAddressSync(
          emissionsMint.publicKey,
          depositor,
        ),
        depositor,
        liquidityVault: bank.liquidityVault,
        tokenProgram: TOKEN_PROGRAM_ID,
      })
      .instruction();

    await expectFailedTxWithError(
      async () => {
        await provider.sendAndConfirm(new Transaction().add(ix));
      },
      "InvalidEmissionsMint",
      6097,
    );
  });

  it("emissions deposit succeeds even if legacy emissions_mint differs", async () => {
    await setBankState({ operational: undefined });
    const bank = await program.account.bank.fetch(bankKeypairA.publicKey);
    const legacyEmissionsMint = Keypair.generate().publicKey;
    const previousEmissionsMint = await setEmissionsDirect(
      provider,
      bankKeypairA.publicKey,
      legacyEmissionsMint,
    );

    try {
      const fundingAccount = getAssociatedTokenAddressSync(
        ecosystem.tokenAMint.publicKey,
        depositor,
      );
      const amount = new BN(1);
      await provider.sendAndConfirm(
        new Transaction().add(
          createMintToInstruction(
            ecosystem.tokenAMint.publicKey,
            fundingAccount,
            depositor,
            BigInt(amount.toNumber()),
          ),
        ),
      );

      const ix = await lendingPoolEmissionsDeposit(program, {
        bank: bankKeypairA.publicKey,
        mint: bank.mint,
        fundingAccount,
        depositor,
        liquidityVault: bank.liquidityVault,
        amount,
      });
      await provider.sendAndConfirm(new Transaction().add(ix));
    } finally {
      await setEmissionsDirect(
        provider,
        bankKeypairA.publicKey,
        previousEmissionsMint,
      );
    }
  });

  it("emissions deposit fails with wrong liquidity vault", async () => {
    await setBankState({ operational: undefined });
    const bank = await program.account.bank.fetch(bankKeypairA.publicKey);

    const ix = await program.methods
      .lendingPoolEmissionsDeposit(new BN(1))
      .accountsStrict({
        group: marginfiGroup.publicKey,
        bank: bankKeypairA.publicKey,
        mint: bank.mint,
        emissionsFundingAccount: bank.feeVault,
        depositor,
        liquidityVault: bank.feeVault,
        tokenProgram: TOKEN_PROGRAM_ID,
      })
      .instruction();

    await expectFailedTxWithError(
      async () => {
        await provider.sendAndConfirm(new Transaction().add(ix));
      },
      "InvalidLiquidityVault",
      6094,
    );
  });

  it("emissions deposit fails when protocol is paused", async () => {
    await setBankState({ operational: undefined });
    const bank = await program.account.bank.fetch(bankKeypairA.publicKey);
    const ix = await lendingPoolEmissionsDeposit(program, {
      bank: bankKeypairA.publicKey,
      mint: bank.mint,
      fundingAccount: bank.feeVault,
      depositor,
      liquidityVault: bank.liquidityVault,
      amount: new BN(1),
    });

    await setProtocolPaused(true);
    try {
      await expectFailedTxWithError(
        async () => {
          await provider.sendAndConfirm(new Transaction().add(ix));
        },
        "ProtocolPaused",
        6080,
      );
    } finally {
      await setProtocolPaused(false);
    }
  });
});

describe("Same-bank deposit - T22", () => {
  let program: Program<Marginfi>;
  let provider: BankrunProvider;
  let depositor: PublicKey;

  const T22_DECIMALS = 6;
  const t22Mint = Keypair.generate();
  const t22BankKeypair = Keypair.generate();
  const t22Oracle = Keypair.generate();
  const t22Feed = Keypair.generate();

  const createT22MintWithExtensions = async (
    mintKeypair: Keypair,
    decimals: number,
    opts?: {
      transferFee?: { feeBasisPoints: number; maxFee: bigint };
      transferHook?: { hookProgramId: PublicKey };
      freezeAuthority?: PublicKey;
    },
  ) => {
    const extensions: ExtensionType[] = [];
    if (opts?.transferFee) extensions.push(ExtensionType.TransferFeeConfig);
    if (opts?.transferHook) extensions.push(ExtensionType.TransferHook);

    const mintLen = getMintLen(extensions);
    const rent = await provider.connection.getMinimumBalanceForRentExemption(
      mintLen,
    );
    const payer = bankrunContext.payer.publicKey;

    const tx = new Transaction();
    tx.add(
      SystemProgram.createAccount({
        fromPubkey: payer,
        newAccountPubkey: mintKeypair.publicKey,
        space: mintLen,
        lamports: rent,
        programId: TOKEN_2022_PROGRAM_ID,
      }),
    );
    if (opts?.transferFee) {
      tx.add(
        createInitializeTransferFeeConfigInstruction(
          mintKeypair.publicKey,
          payer,
          payer,
          opts.transferFee.feeBasisPoints,
          opts.transferFee.maxFee,
          TOKEN_2022_PROGRAM_ID,
        ),
      );
    }
    if (opts?.transferHook) {
      tx.add(
        createInitializeTransferHookInstruction(
          mintKeypair.publicKey,
          payer,
          opts.transferHook.hookProgramId,
          TOKEN_2022_PROGRAM_ID,
        ),
      );
    }
    tx.add(
      createInitializeMint2Instruction(
        mintKeypair.publicKey,
        decimals,
        payer,
        opts?.freezeAuthority ?? null,
        TOKEN_2022_PROGRAM_ID,
      ),
    );

    await processBankrunTransaction(bankrunContext, tx, [
      bankrunContext.payer,
      mintKeypair,
    ]);
  };

  const addT22Bank = async (mint: Keypair, bank: Keypair) => {
    const bankConfig = defaultBankConfig();
    await groupAdmin.mrgnProgram.provider.sendAndConfirm(
      new Transaction().add(
        await groupAdmin.mrgnProgram.methods
          .lendingPoolAddBank({
            ...bankConfig,
            liabilityWeightMaint: bankConfig.liabilityWeightMain,
            pad0: [0, 0, 0, 0, 0, 0],
          })
          .accounts({
            marginfiGroup: marginfiGroup.publicKey,
            feePayer: groupAdmin.wallet.publicKey,
            bankMint: mint.publicKey,
            bank: bank.publicKey,
            tokenProgram: TOKEN_2022_PROGRAM_ID,
          })
          .instruction(),
      ),
      [bank],
    );
  };

  before(async () => {
    program = bankrunProgram;
    provider = bankRunProvider;
    depositor = bankrunContext.payer.publicKey;
  });

  it("emissions deposit succeeds with inactive T22 extensions (fee=0, hook=null)", async () => {
    await createT22MintWithExtensions(t22Mint, T22_DECIMALS, {
      transferFee: { feeBasisPoints: 0, maxFee: BigInt(0) },
      transferHook: { hookProgramId: PublicKey.default },
      freezeAuthority: depositor,
    });

    const banksClient = bankrunContext.banksClient;
    await createBankrunPythFeedAccount(
      bankrunContext,
      banksClient,
      t22Feed,
      PYTH_RECEIVER_PROGRAM_ID,
    );
    await createBankrunPythOracleAccount(
      bankrunContext,
      banksClient,
      t22Oracle,
      PYTH_RECEIVER_PROGRAM_ID,
    );
    await setPythPullOraclePrice(
      bankrunContext,
      banksClient,
      t22Oracle.publicKey,
      t22Feed.publicKey,
      1.0,
      T22_DECIMALS,
      ORACLE_CONF_INTERVAL,
      PYTH_RECEIVER_PROGRAM_ID,
    );
    await addT22Bank(t22Mint, t22BankKeypair);
    await groupAdmin.mrgnProgram.provider.sendAndConfirm(
      new Transaction().add(
        await configureBankOracle(groupAdmin.mrgnProgram, {
          bank: t22BankKeypair.publicKey,
          type: ORACLE_SETUP_PYTH_PUSH,
          oracle: t22Oracle.publicKey,
        }),
      ),
    );

    const ata = getAssociatedTokenAddressSync(
      t22Mint.publicKey,
      depositor,
      false,
      TOKEN_2022_PROGRAM_ID,
    );
    await provider.sendAndConfirm(
      new Transaction().add(
        createAssociatedTokenAccountInstruction(
          depositor,
          ata,
          depositor,
          t22Mint.publicKey,
          TOKEN_2022_PROGRAM_ID,
        ),
        createMintToInstruction(
          t22Mint.publicKey,
          ata,
          depositor,
          BigInt(200 * 10 ** T22_DECIMALS),
          [],
          TOKEN_2022_PROGRAM_ID,
        ),
      ),
    );

    const userAccountKeypair = Keypair.generate();
    await provider.sendAndConfirm(
      new Transaction().add(
        await accountInit(program, {
          marginfiGroup: marginfiGroup.publicKey,
          marginfiAccount: userAccountKeypair.publicKey,
          authority: depositor,
          feePayer: depositor,
        }),
      ),
      [userAccountKeypair],
    );
    await provider.sendAndConfirm(
      new Transaction().add(
        await program.methods
          .lendingAccountDeposit(new BN(100 * 10 ** T22_DECIMALS), false)
          .accounts({
            marginfiAccount: userAccountKeypair.publicKey,
            bank: t22BankKeypair.publicKey,
            signerTokenAccount: ata,
            tokenProgram: TOKEN_2022_PROGRAM_ID,
          })
          .remainingAccounts([
            { pubkey: t22Mint.publicKey, isSigner: false, isWritable: false },
          ])
          .instruction(),
      ),
    );
    await setEmissionsDirect(
      provider,
      t22BankKeypair.publicKey,
      t22Mint.publicKey,
    );

    const bankKey = t22BankKeypair.publicKey;
    const bankBefore = await program.account.bank.fetch(bankKey);

    const [sharesBefore, shareValueBefore] = [
      bankBefore.totalAssetShares,
      bankBefore.assetShareValue,
    ];

    const [liquidityVault] = deriveLiquidityVault(program.programId, bankKey);
    const liquidityVaultBefore = await getTokenBalance(
      provider,
      liquidityVault,
    );

    const depositorAmount = 50;
    const emissionsDepositAmount = depositorAmount * 10 ** T22_DECIMALS;

    const emissionsIx = await program.methods
      .lendingPoolEmissionsDeposit(new BN(emissionsDepositAmount))
      .accounts({
        bank: bankKey,
        depositor: depositor,
        emissionsFundingAccount: ata,
        tokenProgram: TOKEN_2022_PROGRAM_ID,
      })
      .instruction();

    await provider.sendAndConfirm(new Transaction().add(emissionsIx));

    // Fetch after state
    const bankAfter = await program.account.bank.fetch(bankKey);
    const [sharesAfter, shareValueAfter] = [
      bankAfter.totalAssetShares,
      bankAfter.assetShareValue,
    ];
    const liquidityVaultAfter = await getTokenBalance(provider, liquidityVault);

    assertSameBankDeposit(
      sharesBefore,
      sharesAfter,
      shareValueBefore,
      shareValueAfter,
      liquidityVaultBefore,
      liquidityVaultAfter,
      emissionsDepositAmount,
    );
  });

  it("emissions deposit fails with nonzero transfer fee", async () => {
    const [feeMint, feeBank] = [Keypair.generate(), Keypair.generate()];

    await createT22MintWithExtensions(feeMint, T22_DECIMALS, {
      transferFee: { feeBasisPoints: 100, maxFee: BigInt(1_000_000) },
    });
    await addT22Bank(feeMint, feeBank);
    await setEmissionsDirect(provider, feeBank.publicKey, feeMint.publicKey);

    const ix = await program.methods
      .lendingPoolEmissionsDeposit(new BN(1))
      .accounts({
        bank: feeBank.publicKey,
        depositor,
        emissionsFundingAccount: getAssociatedTokenAddressSync(
          feeMint.publicKey,
          depositor,
          false,
          TOKEN_2022_PROGRAM_ID,
        ),
        tokenProgram: TOKEN_2022_PROGRAM_ID,
      })
      .instruction();

    await expectFailedTxWithError(
      async () => {
        await provider.sendAndConfirm(new Transaction().add(ix));
      },
      "InvalidTransfer",
      6004,
    );
  });

  it("emissions deposit fails with active transfer hook", async () => {
    const [hookMint, hookBank] = [Keypair.generate(), Keypair.generate()];

    await createT22MintWithExtensions(hookMint, T22_DECIMALS, {
      transferHook: { hookProgramId: Keypair.generate().publicKey },
    });
    await addT22Bank(hookMint, hookBank);
    await setEmissionsDirect(provider, hookBank.publicKey, hookMint.publicKey);

    const ix = await program.methods
      .lendingPoolEmissionsDeposit(new BN(1))
      .accounts({
        bank: hookBank.publicKey,
        depositor,
        emissionsFundingAccount: getAssociatedTokenAddressSync(
          hookMint.publicKey,
          depositor,
          false,
          TOKEN_2022_PROGRAM_ID,
        ),
        tokenProgram: TOKEN_2022_PROGRAM_ID,
      })
      .instruction();

    await expectFailedTxWithError(
      async () => {
        await provider.sendAndConfirm(new Transaction().add(ix));
      },
      "InvalidTransfer",
      6004,
    );
  });

  it("emissions deposit fails when bank has no depositors", async () => {
    const [emptyMint, emptyBank] = [Keypair.generate(), Keypair.generate()];

    await createT22MintWithExtensions(emptyMint, T22_DECIMALS);
    await addT22Bank(emptyMint, emptyBank);

    const emptyFundingAta = getAssociatedTokenAddressSync(
      emptyMint.publicKey,
      depositor,
      false,
      TOKEN_2022_PROGRAM_ID,
    );
    await provider.sendAndConfirm(
      new Transaction().add(
        createAssociatedTokenAccountInstruction(
          depositor,
          emptyFundingAta,
          depositor,
          emptyMint.publicKey,
          TOKEN_2022_PROGRAM_ID,
        ),
        createMintToInstruction(
          emptyMint.publicKey,
          emptyFundingAta,
          depositor,
          BigInt(1),
          [],
          TOKEN_2022_PROGRAM_ID,
        ),
      ),
    );

    const ix = await program.methods
      .lendingPoolEmissionsDeposit(new BN(1))
      .accounts({
        bank: emptyBank.publicKey,
        depositor,
        emissionsFundingAccount: emptyFundingAta,
        tokenProgram: TOKEN_2022_PROGRAM_ID,
      })
      .instruction();

    await expectFailedTxWithError(
      async () => {
        await provider.sendAndConfirm(new Transaction().add(ix));
      },
      "EmissionsUpdateError",
      6034,
    );
  });
});
