import { BN, Program } from "@coral-xyz/anchor";
import { Keypair, PublicKey, Transaction } from "@solana/web3.js";
import { createMintToInstruction, getAssociatedTokenAddressSync } from "@solana/spl-token";
import { Marginfi } from "../target/types/marginfi";
import {
  bankKeypairUsdc,
  bankrunContext,
  bankrunProgram,
  bankRunProvider,
  ecosystem,
} from "./rootHooks";
import { assert } from "chai";
import { getTokenBalance } from "./utils/genericTests";
import { deriveLiquidityVault } from "./utils/pdas";
import { lendingPoolEmissionsDeposit } from "./utils/group-instructions";
import { setEmissionsDirect } from "./utils/tools";
import { expectFailedTxWithError } from "./utils/genericTests";
import { wrappedI80F48toBigNumber } from "@mrgnlabs/mrgn-common";
import { BankrunProvider } from "anchor-bankrun";
import { createMintBankrun } from "./utils/mocks";

let program: Program<Marginfi>;
let provider: BankrunProvider;
let depositor: PublicKey;
let emissionsMint: PublicKey;

describe("Same-bank deposit", () => {
  before(async () => {
    program = bankrunProgram;
    provider = bankRunProvider;
    depositor = bankrunContext.payer.publicKey;

    const bank = await program.account.bank.fetch(bankKeypairUsdc.publicKey);
    emissionsMint = await setEmissionsDirect(provider, bankKeypairUsdc.publicKey, bank.mint);
  });

  after(async () => {
    // Clean up
    await setEmissionsDirect(provider, bankKeypairUsdc.publicKey, emissionsMint);
  });

  it("deposit same-mint emissions updates share value", async () => {

    // Mint 50 USDC to ATA owned by the bankrun payer
    const depositorAmount = 50;
    const fundingAta = getAssociatedTokenAddressSync(
      ecosystem.usdcMint.publicKey,
      depositor,
    );
    let fundTx = new Transaction();
    fundTx.add(
      createMintToInstruction(
        ecosystem.usdcMint.publicKey,
        fundingAta,
        depositor,
        BigInt(depositorAmount * 10 ** ecosystem.usdcDecimals)
      )
    );
    await provider.sendAndConfirm(fundTx);

    // Snapshot bank and liquidity vault
    const bankBefore = await program.account.bank.fetch(bankKeypairUsdc.publicKey);
    const sharesBefore = bankBefore.totalAssetShares;
    const shareValueBefore = bankBefore.assetShareValue;

    const [liquidityVault] = deriveLiquidityVault(program.programId, bankKeypairUsdc.publicKey);
    const liquidityVaultBefore = await getTokenBalance(provider, liquidityVault);

    // Emissions deposit of 50 USDC from bankrun payer into liquidity vault
    const emissionsDepositAmount = depositorAmount * 10 ** ecosystem.usdcDecimals;
    const ix = await lendingPoolEmissionsDeposit(program, {
      bank: bankKeypairUsdc.publicKey,
      emissionsMint: bankBefore.mint,
      fundingAccount: fundingAta,
      depositor: depositor,
      liquidityVault: liquidityVault,
      amount: new BN(emissionsDepositAmount),
    });

    let tx = new Transaction().add(ix);
    await provider.sendAndConfirm(tx);

    // Fetch after state
    const bankAfter = await program.account.bank.fetch(bankKeypairUsdc.publicKey);
    const sharesAfter = bankAfter.totalAssetShares;
    const shareValueAfter = bankAfter.assetShareValue;

    const liquidityVaultAfter = await getTokenBalance(provider, liquidityVault);

    // Compute total deposited
    const totalAssetSharesBefore = wrappedI80F48toBigNumber(sharesBefore).toNumber();
    const assetShareValueBefore = wrappedI80F48toBigNumber(shareValueBefore).toNumber();
    const totalDeposited = totalAssetSharesBefore * assetShareValueBefore;
    const emissionsNative = Number(emissionsDepositAmount.toString());
    const multiplier = 1 + emissionsNative / totalDeposited;

    // Assertions
    const sharesBeforeStr = wrappedI80F48toBigNumber(sharesBefore).toString();
    const sharesAfterStr = wrappedI80F48toBigNumber(sharesAfter).toString();
    assert.equal(sharesAfterStr, sharesBeforeStr, "total asset shares should be unchanged");

    const beforeValue = wrappedI80F48toBigNumber(shareValueBefore).toNumber();
    const afterValue = wrappedI80F48toBigNumber(shareValueAfter).toNumber();
    assert.approximately(afterValue, beforeValue * multiplier, beforeValue * 10 ** -10);

    assert.equal(liquidityVaultAfter - liquidityVaultBefore, emissionsDepositAmount);
  });

  it("emissions deposit with mismatched mint fails", async () => {

    // Mint 50 USDC to ATA owned by the bankrun payer
    const depositorAmount = 50;
    const fundingAta = getAssociatedTokenAddressSync(
      ecosystem.usdcMint.publicKey,
      depositor,
    );

    let fundTx = new Transaction();
    fundTx.add(
      createMintToInstruction(
        ecosystem.usdcMint.publicKey,
        fundingAta,
        depositor,
        BigInt(depositorAmount * 10 ** ecosystem.usdcDecimals)
      )
    );
    await provider.sendAndConfirm(fundTx);

    const emissionsMint = Keypair.generate();
    await createMintBankrun(
      provider.context,
      provider.wallet.payer,
      9,
      emissionsMint,
    );

    await setEmissionsDirect(provider, bankKeypairUsdc.publicKey, emissionsMint.publicKey);

    const bank = await program.account.bank.fetch(bankKeypairUsdc.publicKey);

    // Emissions deposit of 50 USDC from bankrun payer into liquidity vault
    const emissionsDepositAmount = depositorAmount * 10 ** ecosystem.usdcDecimals;
    const ix = await lendingPoolEmissionsDeposit(program, {
      bank: bankKeypairUsdc.publicKey,
      emissionsMint: bank.emissionsMint,
      fundingAccount: fundingAta,
      depositor: depositor,
      liquidityVault: bank.liquidityVault,
      amount: new BN(emissionsDepositAmount),
    });
    let tx = new Transaction().add(ix);
    await expectFailedTxWithError(async () => { await provider.sendAndConfirm(tx) }, "InvalidEmissionsMint", 6097);
  });
});
