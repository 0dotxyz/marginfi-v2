import { BN } from "@coral-xyz/anchor";
import {
  Keypair,
  PublicKey,
  Transaction,
} from "@solana/web3.js";

import { USER_ACCOUNT_SL, MockUser } from "./utils/mocks";
import {
  bankrunContext,
  bankrunProgram,
  bankRunProvider,
  ecosystem,
  oracles,
  solendAccounts,
  solendGroup,
  users,
  groupAdmin,
  globalProgramAdmin,
  SOLEND_USDC_BANK,
  SOLEND_TOKEN_A_BANK,
  SOLEND_USDC_RESERVE,
  SOLEND_TOKEN_A_RESERVE,
  SOLEND_MARKET,
  SOLEND_USDC_COLLATERAL_MINT,
  SOLEND_TOKEN_A_COLLATERAL_MINT,
} from "./rootHooks";
import {
  makeAddSolendBankIx,
  makeSolendInitObligationIx,
} from "./utils/solend-instructions";
import { defaultSolendBankConfig } from "./utils/solend-utils";
import { ASSET_TAG_SOLEND } from "./utils/types";
import {
  deriveBankWithSeed,
  deriveSolendObligation,
  deriveLiquidityVaultAuthority,
} from "./utils/pdas";
import { processBankrunTransaction } from "./utils/tools";
import {
  assertBankrunTxFailed,
  assertKeysEqual,
  assertI80F48Equal,
} from "./utils/genericTests";
import { assert } from "chai";
import { getTokenBalance } from "./utils/genericTests";
import { groupInitialize } from "./utils/group-instructions";
import {
  createMintToInstruction,
  TOKEN_PROGRAM_ID,
  createAssociatedTokenAccountIdempotentInstruction,
  getAssociatedTokenAddressSync,
} from "@solana/spl-token";
import { accountInit } from "./utils/user-instructions";
import { parseReserve } from "./utils/solend-sdk/state/reserve";

const INIT_OBLIGATION_DEPOSIT_AMOUNT_USDC = 100;
const INIT_OBLIGATION_DEPOSIT_AMOUNT_TOKEN_A = 100;

describe("sl05: Solend - MarginFi Integration", () => {
  let userA: MockUser;
  let usdcBank: PublicKey;
  let tokenABank: PublicKey;

  before(async () => {
    userA = users[0];
  });

  it("(admin) Initialize Solend marginfi group", async () => {
    const tx = new Transaction().add(
      await groupInitialize(groupAdmin.mrgnBankrunProgram, {
        marginfiGroup: solendGroup.publicKey,
        admin: groupAdmin.wallet.publicKey,
      })
    );

    await processBankrunTransaction(
      bankrunContext,
      tx,
      [groupAdmin.wallet, solendGroup],
      false,
      false
    );
  });

  it("(users 0/1) Initialize marginfi accounts", async () => {
    for (let i = 0; i < 2; i++) {
      const user = users[i];
      const kp = Keypair.generate();

      user.accounts.set(USER_ACCOUNT_SL, kp.publicKey);

      const tx = new Transaction().add(
        await accountInit(user.mrgnBankrunProgram, {
          marginfiGroup: solendGroup.publicKey,
          marginfiAccount: kp.publicKey,
          authority: user.wallet.publicKey,
          feePayer: user.wallet.publicKey,
        })
      );

      await processBankrunTransaction(
        bankrunContext,
        tx,
        [user.wallet, kp],
        false,
        false
      );
    }
  });

  it("(admin) Add Solend bank (USDC) - happy path", async () => {
    const config = defaultSolendBankConfig(oracles.usdcOracle.publicKey);

    const seed = new BN(1);
    const [bankKey] = deriveBankWithSeed(
      bankrunProgram.programId,
      solendGroup.publicKey,
      ecosystem.usdcMint.publicKey,
      seed
    );

    const addBankIx = await makeAddSolendBankIx(
      groupAdmin.mrgnBankrunProgram,
      {
        group: solendGroup.publicKey,
        feePayer: groupAdmin.wallet.publicKey,
        bankMint: ecosystem.usdcMint.publicKey,
        integrationAcc1: solendAccounts.get(SOLEND_USDC_RESERVE)!,
        solendMarket: solendAccounts.get(SOLEND_MARKET)!,
        oracle: oracles.usdcOracle.publicKey,
      },
      {
        config,
        seed,
      }
    );

    const tx = new Transaction().add(addBankIx);
    await processBankrunTransaction(
      bankrunContext,
      tx,
      [groupAdmin.wallet],
      false,
      true
    );

    solendAccounts.set(SOLEND_USDC_BANK, bankKey);
    usdcBank = bankKey;

    const bank = await bankrunProgram.account.bank.fetch(bankKey);
    assertKeysEqual(bank.mint, ecosystem.usdcMint.publicKey);
    assert.equal(bank.config.assetTag, ASSET_TAG_SOLEND);
    assertKeysEqual(bank.group, solendGroup.publicKey);

    const usdcReserve = solendAccounts.get(SOLEND_USDC_RESERVE)!;
    assert.equal(bank.mintDecimals, ecosystem.usdcDecimals);
    assertKeysEqual(bank.config.oracleKeys[0], oracles.usdcOracle.publicKey);
    assertKeysEqual(bank.config.oracleKeys[1], usdcReserve);
    assertKeysEqual(bank.integrationAcc1, usdcReserve);

    const [expectedObligation] = deriveSolendObligation(
      bankrunProgram.programId,
      usdcBank
    );
    assertKeysEqual(bank.integrationAcc2, expectedObligation);
    assert.ok(Object.keys(bank.config.oracleSetup).includes("solendPythPull"));
  });

  it("(admin) Add Token A bank", async () => {
    const config = defaultSolendBankConfig(
      oracles.tokenAOracle.publicKey,
      ecosystem.tokenADecimals
    );

    const seed = new BN(2);
    const [bankKey] = deriveBankWithSeed(
      bankrunProgram.programId,
      solendGroup.publicKey,
      ecosystem.tokenAMint.publicKey,
      seed
    );

    const addBankIx = await makeAddSolendBankIx(
      groupAdmin.mrgnBankrunProgram,
      {
        group: solendGroup.publicKey,
        feePayer: groupAdmin.wallet.publicKey,
        bankMint: ecosystem.tokenAMint.publicKey,
        integrationAcc1: solendAccounts.get(SOLEND_TOKEN_A_RESERVE)!,
        solendMarket: solendAccounts.get(SOLEND_MARKET)!,
        oracle: oracles.tokenAOracle.publicKey,
      },
      {
        config,
        seed,
      }
    );

    const tx = new Transaction().add(addBankIx);
    await processBankrunTransaction(
      bankrunContext,
      tx,
      [groupAdmin.wallet],
      false,
      true
    );

    solendAccounts.set(SOLEND_TOKEN_A_BANK, bankKey);
    tokenABank = bankKey;

    const bank = await bankrunProgram.account.bank.fetch(bankKey);
    assertKeysEqual(bank.mint, ecosystem.tokenAMint.publicKey);
    assert.equal(bank.config.assetTag, ASSET_TAG_SOLEND);
    assertKeysEqual(bank.group, solendGroup.publicKey);

    const tokenAReserve = solendAccounts.get(SOLEND_TOKEN_A_RESERVE)!;
    assert.equal(bank.mintDecimals, ecosystem.tokenADecimals);
    assertKeysEqual(bank.config.oracleKeys[0], oracles.tokenAOracle.publicKey);
    assertKeysEqual(bank.config.oracleKeys[1], tokenAReserve);
    assertKeysEqual(bank.integrationAcc1, tokenAReserve);

    const [expectedTokenAObligation] = deriveSolendObligation(
      bankrunProgram.programId,
      tokenABank
    );
    assertKeysEqual(bank.integrationAcc2, expectedTokenAObligation);
    assert.ok(Object.keys(bank.config.oracleSetup).includes("solendPythPull"));
  });

  it("(user 0) Tries to add bank - admin only, should fail", async () => {
    const config = defaultSolendBankConfig(oracles.usdcOracle.publicKey);

    const seed = new BN(3);

    const addBankIx = await makeAddSolendBankIx(
      userA.mrgnBankrunProgram,
      {
        group: solendGroup.publicKey,
        feePayer: userA.wallet.publicKey,
        bankMint: ecosystem.usdcMint.publicKey,
        integrationAcc1: solendAccounts.get(SOLEND_USDC_RESERVE)!,
        solendMarket: solendAccounts.get(SOLEND_MARKET)!,
        oracle: oracles.usdcOracle.publicKey,
      },
      {
        config,
        seed,
      }
    );

    const tx = new Transaction().add(addBankIx);
    const result = await processBankrunTransaction(
      bankrunContext,
      tx,
      [userA.wallet],
      true,
      false
    );

    assertBankrunTxFailed(result, 6042); // MarginfiError::Unauthorized
  });

  it("(user 0) Tries to init obligation with insufficient deposit - should fail", async () => {
    const fundAmount = new BN(5);
    const fundTx = new Transaction().add(
      createMintToInstruction(
        ecosystem.usdcMint.publicKey,
        userA.usdcAccount,
        globalProgramAdmin.wallet.publicKey,
        fundAmount.toNumber()
      )
    );
    await processBankrunTransaction(bankrunContext, fundTx, [
      globalProgramAdmin.wallet,
    ]);

    const collateralMint = solendAccounts.get(SOLEND_USDC_COLLATERAL_MINT)!;

    const [liquidityVaultAuthority] = deriveLiquidityVaultAuthority(
      bankrunProgram.programId,
      usdcBank
    );

    const userCollateral = getAssociatedTokenAddressSync(
      collateralMint,
      liquidityVaultAuthority,
      true
    );

    const createAtaIx = createAssociatedTokenAccountIdempotentInstruction(
      userA.wallet.publicKey,
      userCollateral,
      liquidityVaultAuthority,
      collateralMint,
      TOKEN_PROGRAM_ID
    );

    const initObligationIx = await makeSolendInitObligationIx(
      userA.mrgnBankrunProgram,
      {
        feePayer: userA.wallet.publicKey,
        bank: usdcBank,
        signerTokenAccount: userA.usdcAccount,
        lendingMarket: solendAccounts.get(SOLEND_MARKET)!,
        pythPrice: oracles.usdcOracleFeed.publicKey,
      },
      {
        amount: new BN(5), // Less than minimum of 10
      }
    );

    const tx = new Transaction().add(createAtaIx, initObligationIx);
    const result = await processBankrunTransaction(
      bankrunContext,
      tx,
      [userA.wallet],
      true,
      false
    );

    assertBankrunTxFailed(result, 0x1841); // ObligationInitDepositInsufficient
  });

  it("(user 0) Tries to init obligation without creating ATA - should fail", async () => {
    // Try to init obligation without creating the ATA
    const initObligationIx = await makeSolendInitObligationIx(
      userA.mrgnBankrunProgram,
      {
        feePayer: userA.wallet.publicKey,
        bank: usdcBank,
        signerTokenAccount: userA.usdcAccount,
        lendingMarket: solendAccounts.get(SOLEND_MARKET)!,
        pythPrice: oracles.usdcOracleFeed.publicKey,
      },
      {
        amount: new BN(10_000),
      }
    );

    const tx = new Transaction().add(initObligationIx);
    const result = await processBankrunTransaction(
      bankrunContext,
      tx,
      [userA.wallet],
      true,
      false
    );

    assertBankrunTxFailed(result, "invalid account data");
  });

  it("(admin) Initialize Solend obligation for USDC bank", async () => {
    const initialBalance = await getTokenBalance(
      bankRunProvider,
      groupAdmin.usdcAccount
    );

    const fundAmount = new BN(100 * 10 ** ecosystem.usdcDecimals); // 100 USDC
    const fundTx = new Transaction().add(
      createMintToInstruction(
        ecosystem.usdcMint.publicKey,
        groupAdmin.usdcAccount,
        globalProgramAdmin.wallet.publicKey,
        fundAmount.toNumber()
      )
    );
    await processBankrunTransaction(bankrunContext, fundTx, [
      globalProgramAdmin.wallet,
    ]);

    const usdcReservePubkey = solendAccounts.get(SOLEND_USDC_RESERVE)!;
    const reserveAccountBefore = await bankrunContext.banksClient.getAccount(
      usdcReservePubkey
    );
    const reserveBefore = parseReserve(usdcReservePubkey, {
      ...reserveAccountBefore,
      data: Buffer.from(reserveAccountBefore.data),
    });
    const liquidityBefore =
      reserveBefore.info.liquidity.availableAmount.toNumber();

    const collateralMint = solendAccounts.get(SOLEND_USDC_COLLATERAL_MINT)!;

    const [liquidityVaultAuthority] = deriveLiquidityVaultAuthority(
      bankrunProgram.programId,
      usdcBank
    );

    const userCollateral = getAssociatedTokenAddressSync(
      collateralMint,
      liquidityVaultAuthority,
      true
    );

    const createAtaIx = createAssociatedTokenAccountIdempotentInstruction(
      groupAdmin.wallet.publicKey,
      userCollateral,
      liquidityVaultAuthority,
      collateralMint,
      TOKEN_PROGRAM_ID
    );

    const initObligationIx = await makeSolendInitObligationIx(
      groupAdmin.mrgnBankrunProgram,
      {
        feePayer: groupAdmin.wallet.publicKey,
        bank: usdcBank,
        signerTokenAccount: groupAdmin.usdcAccount,
        lendingMarket: solendAccounts.get(SOLEND_MARKET)!,
        pythPrice: oracles.usdcOracleFeed.publicKey,
      },
      {
        amount: new BN(INIT_OBLIGATION_DEPOSIT_AMOUNT_USDC),
      }
    );

    const tx = new Transaction().add(createAtaIx, initObligationIx);
    await processBankrunTransaction(
      bankrunContext,
      tx,
      [groupAdmin.wallet],
      false,
      false
    );

    const adminBalance = await getTokenBalance(
      bankRunProvider,
      groupAdmin.usdcAccount
    );
    const expectedBalance =
      initialBalance +
      fundAmount.toNumber() -
      INIT_OBLIGATION_DEPOSIT_AMOUNT_USDC;
    assert.equal(adminBalance.toString(), expectedBalance.toString());

    const bank = await bankrunProgram.account.bank.fetch(usdcBank);
    const [expectedObligation] = deriveSolendObligation(
      bankrunProgram.programId,
      usdcBank
    );
    assertKeysEqual(bank.integrationAcc2, expectedObligation);
    assertI80F48Equal(bank.totalAssetShares, 0);
    const obligationAccount = await bankrunContext.banksClient.getAccount(
      expectedObligation
    );
    assert.ok(obligationAccount);

    const { parseObligation } = await import("./utils/solend-sdk");
    const obligationData = parseObligation(expectedObligation, {
      ...obligationAccount,
      data: Buffer.from(obligationAccount.data),
    });

    assert.equal(obligationData.info.deposits.length, 1);
    assertKeysEqual(
      obligationData.info.deposits[0].depositReserve,
      solendAccounts.get(SOLEND_USDC_RESERVE)!
    );

    const depositAmount =
      obligationData.info.deposits[0].depositedAmount.toNumber();

    // We only care that the obligation has a non-zero deposit to prevent rounding/closing issues
    // The exact cToken amount doesn't matter for this test setup
    assert.ok(depositAmount > 0);

    const reserveAccountAfter = await bankrunContext.banksClient.getAccount(
      usdcReservePubkey
    );
    const reserveAfter = parseReserve(usdcReservePubkey, {
      ...reserveAccountAfter,
      data: Buffer.from(reserveAccountAfter.data),
    });
    const liquidityAfter =
      reserveAfter.info.liquidity.availableAmount.toNumber();

    assert.equal(
      liquidityAfter - liquidityBefore,
      INIT_OBLIGATION_DEPOSIT_AMOUNT_USDC
    );
  });

  it("(admin) Initialize Solend obligation for Token A bank", async () => {
    const initialBalance = await getTokenBalance(
      bankRunProvider,
      groupAdmin.tokenAAccount
    );

    const fundAmount = new BN(1 * 10 ** ecosystem.tokenADecimals);
    const fundTx = new Transaction().add(
      createMintToInstruction(
        ecosystem.tokenAMint.publicKey,
        groupAdmin.tokenAAccount,
        globalProgramAdmin.wallet.publicKey,
        fundAmount.toNumber()
      )
    );
    await processBankrunTransaction(bankrunContext, fundTx, [
      globalProgramAdmin.wallet,
    ]);

    const tokenAReservePubkey = solendAccounts.get(SOLEND_TOKEN_A_RESERVE)!;
    const reserveAccountBefore = await bankrunContext.banksClient.getAccount(
      tokenAReservePubkey
    );
    const reserveBefore = parseReserve(tokenAReservePubkey, {
      ...reserveAccountBefore,
      data: Buffer.from(reserveAccountBefore.data),
    });
    const liquidityBefore =
      reserveBefore.info.liquidity.availableAmount.toNumber();

    const collateralMint = solendAccounts.get(SOLEND_TOKEN_A_COLLATERAL_MINT)!;

    const [liquidityVaultAuthority] = deriveLiquidityVaultAuthority(
      bankrunProgram.programId,
      tokenABank
    );

    const userCollateral = getAssociatedTokenAddressSync(
      collateralMint,
      liquidityVaultAuthority,
      true
    );

    const createAtaIx = createAssociatedTokenAccountIdempotentInstruction(
      groupAdmin.wallet.publicKey,
      userCollateral,
      liquidityVaultAuthority,
      collateralMint,
      TOKEN_PROGRAM_ID
    );

    const initObligationIx = await makeSolendInitObligationIx(
      groupAdmin.mrgnBankrunProgram,
      {
        feePayer: groupAdmin.wallet.publicKey,
        bank: tokenABank,
        signerTokenAccount: groupAdmin.tokenAAccount,
        lendingMarket: solendAccounts.get(SOLEND_MARKET)!,
        pythPrice: oracles.tokenAOracleFeed.publicKey,
      },
      {
        amount: new BN(INIT_OBLIGATION_DEPOSIT_AMOUNT_TOKEN_A),
      }
    );

    const tx = new Transaction().add(createAtaIx, initObligationIx);
    await processBankrunTransaction(
      bankrunContext,
      tx,
      [groupAdmin.wallet],
      false,
      false
    );

    const adminBalance = await getTokenBalance(
      bankRunProvider,
      groupAdmin.tokenAAccount
    );
    const expectedBalance =
      initialBalance +
      fundAmount.toNumber() -
      INIT_OBLIGATION_DEPOSIT_AMOUNT_TOKEN_A;
    assert.equal(adminBalance.toString(), expectedBalance.toString());

    const bank = await bankrunProgram.account.bank.fetch(tokenABank);
    const [expectedTokenAObligation] = deriveSolendObligation(
      bankrunProgram.programId,
      tokenABank
    );
    assertKeysEqual(bank.integrationAcc2, expectedTokenAObligation);
    assertI80F48Equal(bank.totalAssetShares, 0);

    const obligationAccount = await bankrunContext.banksClient.getAccount(
      expectedTokenAObligation
    );
    assert.ok(obligationAccount);

    const { parseObligation } = await import("./utils/solend-sdk");
    const obligationData = parseObligation(expectedTokenAObligation, {
      ...obligationAccount,
      data: Buffer.from(obligationAccount.data),
    });

    assert.equal(obligationData.info.deposits.length, 1);
    assertKeysEqual(
      obligationData.info.deposits[0].depositReserve,
      solendAccounts.get(SOLEND_TOKEN_A_RESERVE)!
    );

    const depositAmount =
      obligationData.info.deposits[0].depositedAmount.toNumber();

    // We only care that the obligation has a non-zero deposit to prevent rounding/closing issues
    // The exact cToken amount doesn't matter for this test setup
    assert.ok(depositAmount > 0);

    const reserveAccountAfter = await bankrunContext.banksClient.getAccount(
      tokenAReservePubkey
    );
    const reserveAfter = parseReserve(tokenAReservePubkey, {
      ...reserveAccountAfter,
      data: Buffer.from(reserveAccountAfter.data),
    });
    const liquidityAfter =
      reserveAfter.info.liquidity.availableAmount.toNumber();

    assert.equal(
      liquidityAfter - liquidityBefore,
      INIT_OBLIGATION_DEPOSIT_AMOUNT_TOKEN_A
    );
  });
});
