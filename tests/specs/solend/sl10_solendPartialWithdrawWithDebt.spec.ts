import { PublicKey, Transaction, ComputeBudgetProgram } from "@solana/web3.js";
import {
  createMintToInstruction,
  createAssociatedTokenAccountIdempotentInstruction,
  getAssociatedTokenAddressSync,
  TOKEN_PROGRAM_ID,
} from "@solana/spl-token";
import BN from "bn.js";
import { assert } from "chai";
import {
  groupAdmin,
  oracles,
  ecosystem,
  globalProgramAdmin,
  bankrunContext,
  bankrunProgram,
  users,
  solendAccounts,
  SOLEND_USDC_RESERVE,
  SOLEND_MARKET,
  SOLEND_USDC_COLLATERAL_MINT,
} from "../../rootHooks";
import { MockUser } from "../../utils/mocks";
import {
  depositIx,
  borrowIx,
  composeRemainingAccounts,
} from "../../utils/user-instructions";
import {
  makeAddSolendBankIx,
  makeSolendInitObligationIx,
  makeSolendDepositIx,
  makeSolendWithdrawIx,
} from "../../utils/solend-instructions";
import {
  deriveBankWithSeed,
  deriveLiquidityVaultAuthority,
} from "../../utils/pdas";
import { genericMultiBankTestSetup } from "../../genericSetups";
import { defaultSolendBankConfig } from "../../utils/solend-utils";
import {
  processBankrunTransaction as processBankrunTx,
  toBnFromI80,
} from "../../utils/tools";

describe("sl10: Solend partial withdraw with debt", () => {
  const startingSeed = 1000;
  const solendBankSeed = new BN(startingSeed + 1);
  const accountName = "throwaway_account_sl10";

  const obligationInitAmountUsdc = new BN(1000);
  const liquidityAmountLst = new BN(5 * 10 ** ecosystem.lstAlphaDecimals);
  const depositAmountUsdc = new BN(1250 * 10 ** ecosystem.usdcDecimals);
  const borrowAmountLst = new BN(1 * 10 ** ecosystem.lstAlphaDecimals);

  let user: MockUser;
  let userAccount: PublicKey;
  let lstBank: PublicKey;
  let solendUsdcBank: PublicKey;
  let solendReserve: PublicKey;
  let solendMarket: PublicKey;

  const userRiskAccounts = () =>
    composeRemainingAccounts([
      [solendUsdcBank, oracles.usdcOracle.publicKey, solendReserve],
      [lstBank, oracles.pythPullLst.publicKey],
    ]);

  before(async () => {
    const { throwawayGroup, banks } = await genericMultiBankTestSetup(
      1,
      accountName,
      Buffer.from("throwaway_group_sl10_00000000000"),
      startingSeed
    );

    user = users[0];
    userAccount = user.accounts.get(accountName)!;
    const adminAccount = groupAdmin.accounts.get(accountName)!;

    lstBank = banks[0];
    solendReserve = solendAccounts.get(SOLEND_USDC_RESERVE)!;
    solendMarket = solendAccounts.get(SOLEND_MARKET)!;

    [solendUsdcBank] = deriveBankWithSeed(
      bankrunProgram.programId,
      throwawayGroup.publicKey,
      ecosystem.usdcMint.publicKey,
      solendBankSeed
    );

    await processBankrunTx(
      bankrunContext,
      new Transaction().add(
        createMintToInstruction(
          ecosystem.usdcMint.publicKey,
          groupAdmin.usdcAccount,
          globalProgramAdmin.wallet.publicKey,
          obligationInitAmountUsdc.toNumber()
        )
      ),
      [globalProgramAdmin.wallet]
    );

    await processBankrunTx(
      bankrunContext,
      new Transaction().add(
        await makeAddSolendBankIx(
          groupAdmin.mrgnBankrunProgram,
          {
            group: throwawayGroup.publicKey,
            feePayer: groupAdmin.wallet.publicKey,
            bankMint: ecosystem.usdcMint.publicKey,
            integrationAcc1: solendReserve,
            solendMarket,
            oracle: oracles.usdcOracle.publicKey,
          },
          {
            config: defaultSolendBankConfig(oracles.usdcOracle.publicKey),
            seed: solendBankSeed,
          }
        )
      ),
      [groupAdmin.wallet]
    );

    const collateralMint = solendAccounts.get(SOLEND_USDC_COLLATERAL_MINT)!;

    const [liquidityVaultAuthority] = deriveLiquidityVaultAuthority(
      bankrunProgram.programId,
      solendUsdcBank
    );

    const vaultCollateralAta = getAssociatedTokenAddressSync(
      collateralMint,
      liquidityVaultAuthority,
      true
    );

    await processBankrunTx(
      bankrunContext,
      new Transaction().add(
        createAssociatedTokenAccountIdempotentInstruction(
          groupAdmin.wallet.publicKey,
          vaultCollateralAta,
          liquidityVaultAuthority,
          collateralMint,
          TOKEN_PROGRAM_ID
        ),
        await makeSolendInitObligationIx(
          groupAdmin.mrgnBankrunProgram,
          {
            feePayer: groupAdmin.wallet.publicKey,
            bank: solendUsdcBank,
            signerTokenAccount: groupAdmin.usdcAccount,
            lendingMarket: solendMarket,
            pythPrice: oracles.usdcOracleFeed.publicKey,
          },
          {
            amount: obligationInitAmountUsdc,
          }
        )
      ),
      [groupAdmin.wallet]
    );

    await processBankrunTx(
      bankrunContext,
      new Transaction().add(
        await depositIx(groupAdmin.mrgnBankrunProgram, {
          marginfiAccount: adminAccount,
          bank: lstBank,
          tokenAccount: groupAdmin.lstAlphaAccount,
          amount: liquidityAmountLst,
          depositUpToLimit: false,
        })
      ),
      [groupAdmin.wallet]
    );

    await processBankrunTx(
      bankrunContext,
      new Transaction().add(
        createMintToInstruction(
          ecosystem.usdcMint.publicKey,
          user.usdcAccount,
          globalProgramAdmin.wallet.publicKey,
          depositAmountUsdc.toNumber()
        )
      ),
      [globalProgramAdmin.wallet]
    );

    await processBankrunTx(
      bankrunContext,
      new Transaction().add(
        await makeSolendDepositIx(
          user.mrgnBankrunProgram,
          {
            marginfiAccount: userAccount,
            bank: solendUsdcBank,
            signerTokenAccount: user.usdcAccount,
            lendingMarket: solendMarket,
            pythPrice: oracles.usdcOracle.publicKey,
          },
          {
            amount: depositAmountUsdc,
          }
        )
      ),
      [user.wallet]
    );

    await processBankrunTx(
      bankrunContext,
      new Transaction().add(
        await borrowIx(user.mrgnBankrunProgram, {
          marginfiAccount: userAccount,
          bank: lstBank,
          tokenAccount: user.lstAlphaAccount,
          amount: borrowAmountLst,
          remaining: userRiskAccounts(),
        })
      ),
      [user.wallet]
    );
  });

  it("allows a partial Solend withdraw with outstanding debt", async () => {
    const accountBefore =
      await user.mrgnBankrunProgram.account.marginfiAccount.fetch(userAccount);

    const solendBalanceBefore = accountBefore.lendingAccount.balances.find(
      (b) => b.active === 1 && b.bankPk.equals(solendUsdcBank)
    );

    const lstBalanceBefore = accountBefore.lendingAccount.balances.find(
      (b) => b.active === 1 && b.bankPk.equals(lstBank)
    );

    assert.isDefined(solendBalanceBefore);
    assert.isDefined(lstBalanceBefore);

    assert.isTrue(toBnFromI80(lstBalanceBefore!.liabilityShares).gtn(0));

    const sharesBefore = toBnFromI80(solendBalanceBefore!.assetShares);

    const withdrawAmount = sharesBefore.div(new BN(10));
    assert.isTrue(withdrawAmount.gtn(0));

    await processBankrunTx(
      bankrunContext,
      new Transaction().add(
        ComputeBudgetProgram.setComputeUnitLimit({
          units: 400_000,
        }),
        await makeSolendWithdrawIx(
          user.mrgnBankrunProgram,
          {
            marginfiAccount: userAccount,
            bank: solendUsdcBank,
            destinationTokenAccount: user.usdcAccount,
            lendingMarket: solendMarket,
            pythPrice: oracles.usdcOracle.publicKey,
          },
          {
            amount: withdrawAmount,
            withdrawAll: false,
            remaining: userRiskAccounts(),
          }
        )
      ),
      [user.wallet]
    );

    const accountAfter =
      await user.mrgnBankrunProgram.account.marginfiAccount.fetch(userAccount);

    const solendBalanceAfter = accountAfter.lendingAccount.balances.find(
      (b) => b.active === 1 && b.bankPk.equals(solendUsdcBank)
    );

    assert.isDefined(solendBalanceAfter);

    assert.isTrue(
      toBnFromI80(solendBalanceAfter!.assetShares).lt(sharesBefore)
    );
  });
});
