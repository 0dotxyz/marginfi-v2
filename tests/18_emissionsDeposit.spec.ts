import { BN, Program } from "@coral-xyz/anchor";
import { PublicKey, Transaction } from "@solana/web3.js";
import { createMintToInstruction, getAssociatedTokenAddressSync, TOKEN_PROGRAM_ID } from "@solana/spl-token";
import { Marginfi } from "../target/types/marginfi";
import {
  bankKeypairUsdc,
  bankrunContext,
  bankrunProgram,
  bankRunProvider,
  ecosystem,
  users,
  marginfiGroup,
} from "./rootHooks";
import { assert } from "chai";
import { getTokenBalance } from "./utils/genericTests";
import { accountInitPda } from "./utils/user-instructions";
import { deriveMarginfiAccountPda, deriveLiquidityVault } from "./utils/pdas";
import { depositIx } from "./utils/user-instructions";
import { lendingPoolEmissionsDeposit } from "./utils/group-instructions";
import { resetEmissionsDirect, setEmissionsDirect } from "./utils/tools";
import { wrappedI80F48toBigNumber } from "@mrgnlabs/mrgn-common";
import { BankrunProvider } from "anchor-bankrun";

let program: Program<Marginfi>;
let provider: BankrunProvider;
let emissionsMint: PublicKey;

describe("Same-bank deposit", () => {
  before(async () => {
    program = bankrunProgram;
    provider = bankRunProvider;
    emissionsMint = await setEmissionsDirect(provider, bankKeypairUsdc.publicKey);
  });

  after(async () => {
    await resetEmissionsDirect(provider, bankKeypairUsdc.publicKey, emissionsMint);
  });

  it("deposit same-mint emissions updates share value", async () => {

    // Mint 50 USDC to ATA owned by the bankrun payer
    const depositorAmount = 50;
    const fundingAta = getAssociatedTokenAddressSync(
      ecosystem.usdcMint.publicKey,
      bankrunContext.payer.publicKey,
      true,
      TOKEN_PROGRAM_ID
    );
    let fundTx = new Transaction();
    fundTx.add(
      createMintToInstruction(
        ecosystem.usdcMint.publicKey,
        fundingAta,
        bankrunContext.payer.publicKey,
        BigInt(depositorAmount * 10 ** ecosystem.usdcDecimals)
      )
    );
    await provider.sendAndConfirm(fundTx);

    // Prepare two depositors and mint them tokens
    const depositorA = users[2];
    const depositorB = users[3];

    const [mfiA] = deriveMarginfiAccountPda(program.programId, marginfiGroup.publicKey, depositorA.wallet.publicKey, 0);
    const [mfiB] = deriveMarginfiAccountPda(program.programId, marginfiGroup.publicKey, depositorB.wallet.publicKey, 1);

    depositorA.accounts.set("USER_ACCOUNT_PDA", mfiA);
    depositorB.accounts.set("USER_ACCOUNT_PDA", mfiB);

    // Initialize PDAs
    let txA = new Transaction().add(
      await accountInitPda(program, {
        marginfiGroup: marginfiGroup.publicKey,
        marginfiAccount: mfiA,
        authority: depositorA.wallet.publicKey,
        feePayer: depositorA.wallet.publicKey,
        accountIndex: 0,
      })
    );
    await depositorA.mrgnProgram.provider.sendAndConfirm(txA, []);

    let txB = new Transaction().add(
      await accountInitPda(program, {
        marginfiGroup: marginfiGroup.publicKey,
        marginfiAccount: mfiB,
        authority: depositorB.wallet.publicKey,
        feePayer: depositorB.wallet.publicKey,
        accountIndex: 1,
      })
    );
    await depositorB.mrgnProgram.provider.sendAndConfirm(txB, []);

    // Mint USDC to depositor ATAs
    const mintTx = new Transaction();
    const amountA = 40;
    const amountB = 60;
    mintTx.add(
      createMintToInstruction(
        ecosystem.usdcMint.publicKey,
        depositorA.usdcAccount,
        bankrunContext.payer.publicKey,
        BigInt(amountA * 10 ** ecosystem.usdcDecimals)
      )
    );
    mintTx.add(
      createMintToInstruction(
        ecosystem.usdcMint.publicKey,
        depositorB.usdcAccount,
        bankrunContext.payer.publicKey,
        BigInt(amountB * 10 ** ecosystem.usdcDecimals)
      )
    );
    await provider.sendAndConfirm(mintTx);

    // Depositors deposit 40 and 60 USDC respectively
    const depositorAAmount = amountA * 10 ** ecosystem.usdcDecimals;
    const depositorBAmount = amountB * 10 ** ecosystem.usdcDecimals;

    const depositTxA = new Transaction().add(
      await depositIx(depositorA.mrgnProgram, {
        marginfiAccount: mfiA,
        bank: bankKeypairUsdc.publicKey,
        tokenAccount: depositorA.usdcAccount,
        amount: new BN(depositorAAmount),
      })
    );
    await depositorA.mrgnProgram.provider.sendAndConfirm(depositTxA, []);

    const depositTxB = new Transaction().add(
      await depositIx(depositorB.mrgnProgram, {
        marginfiAccount: mfiB,
        bank: bankKeypairUsdc.publicKey,
        tokenAccount: depositorB.usdcAccount,
        amount: new BN(depositorBAmount),
      })
    );
    await depositorB.mrgnProgram.provider.sendAndConfirm(depositTxB, []);

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
      depositor: bankrunContext.payer.publicKey,
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
});
