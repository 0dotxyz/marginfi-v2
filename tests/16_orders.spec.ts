import {
  BN,
  Program,
  Wallet,
} from "@coral-xyz/anchor";
import { bigNumberToWrappedI80F48, TOKEN_PROGRAM_ID, wrappedI80F48toBigNumber } from "@mrgnlabs/mrgn-common";
import { CONF_INTERVAL_MULTIPLE, ORACLE_CONF_INTERVAL } from "./utils/types";
import {
  createMintToInstruction,
  createAssociatedTokenAccountIdempotentInstruction,
  getAssociatedTokenAddressSync,
} from "@solana/spl-token";
import { PublicKey, Transaction } from "@solana/web3.js";
import { assert, expect } from "chai";
import { Marginfi } from "../target/types/marginfi";
import {
  placeOrderIx,
  OrderTriggerArgs,
  composeRemainingAccounts,
  startExecuteOrderIx,
  endExecuteOrderIx,
  closeOrderIx,
  keeperCloseOrderIx,
  setLiquidatorCloseFlagsIx,
  depositIx,
  borrowIx,
  updateEmissionsDestination
} from "./utils/user-instructions";
import { deriveOrderPda, deriveExecuteOrderPda } from "./utils/pdas";
import { refreshOracles as refreshPullOracles } from "./utils/pyth-pull-mocks";
import {
  users,
  marginfiGroup,
  bankKeypairA,
  bankKeypairSol,
  bankKeypairUsdc,
  bankrunProgram,
  bankRunProvider,
  ecosystem,
  oracles,
} from "./rootHooks";
import { MockUser, USER_ACCOUNT } from "./utils/mocks";
import { expectFailedTxWithError, expectFailedTxWithMessage } from "./utils/genericTests";
import { BankrunProvider } from "anchor-bankrun";


let program: Program<Marginfi>;
let provider: BankrunProvider;
let wallet: Wallet;
let keeperUser: MockUser;
let keeperProgram: Program<Marginfi>;
let keeperMarginfiAccount: PublicKey;
let oracleBaseline: { tokenAPrice: number; wsolPrice: number; usdcPrice: number };

describe("orders", () => {

  let user: MockUser;
  let userProgram: Program<Marginfi>;
  let userMarginfiAccount: PublicKey;

  const bankA = bankKeypairA.publicKey; // asset
  const bankSol = bankKeypairSol.publicKey; // asset
  const bankUsdc = bankKeypairUsdc.publicKey; // liability

  const depositA = new BN(5 * 10 ** ecosystem.tokenADecimals);
  const depositSol = new BN(0.5 * 10 ** ecosystem.wsolDecimals);
  const borrowUsdc = new BN(9 * 10 ** ecosystem.usdcDecimals);

  const captureOracleSnapshot = () => {
    oracleBaseline = {
      tokenAPrice: oracles.tokenAPrice,
      wsolPrice: oracles.wsolPrice,
      usdcPrice: oracles.usdcPrice,
    };
  };

  const restoreOracles = async () => {
    if (!oracleBaseline) return;

    oracles.tokenAPrice = oracleBaseline.tokenAPrice;
    oracles.wsolPrice = oracleBaseline.wsolPrice;
    oracles.usdcPrice = oracleBaseline.usdcPrice;

    const now = Math.floor(Date.now() / 1000);
    const slot = new BN(now);
    await refreshPullOracles(oracles, wallet.payer, slot, now);
  };

  const stopLossThreshold = bigNumberToWrappedI80F48(100);
  const takeProfitThreshold = bigNumberToWrappedI80F48(250);
  const highTakeProfit = bigNumberToWrappedI80F48(50);
  const maxSlippage = new BN(100);

  before(async () => {
    // We make changes to the oracle so we need to revert the changes after.
    captureOracleSnapshot();

    provider = bankRunProvider;
    program = bankrunProgram;
    wallet = provider.wallet as Wallet;
    keeperUser = users[1];
    keeperProgram = keeperUser.mrgnProgram as Program<Marginfi>;
    keeperMarginfiAccount = keeperUser.accounts.get(USER_ACCOUNT);

    user = users[0];
    userProgram = user.mrgnProgram as Program<Marginfi>;
    userMarginfiAccount = user.accounts.get(USER_ACCOUNT);

    const fundTx = new Transaction();
    fundTx.add(
      createMintToInstruction(
        ecosystem.tokenAMint.publicKey,
        user.tokenAAccount,
        wallet.publicKey,
        50 * 10 ** ecosystem.tokenADecimals
      )
    );
    fundTx.add(
      createMintToInstruction(
        ecosystem.wsolMint.publicKey,
        user.wsolAccount,
        wallet.publicKey,
        10 * 10 ** ecosystem.wsolDecimals
      )
    );

    await provider.sendAndConfirm(fundTx, [wallet.payer]);

    const depositSolIx = await depositIx(user.mrgnProgram, {
      marginfiAccount: userMarginfiAccount,
      bank: bankSol,
      tokenAccount: user.wsolAccount,
      amount: depositSol,
      depositUpToLimit: false,
    });

    await userProgram.provider.sendAndConfirm(new Transaction().add(depositSolIx));

    const depositAIx = await depositIx(user.mrgnProgram, {
      marginfiAccount: userMarginfiAccount,
      bank: bankA,
      tokenAccount: user.tokenAAccount,
      amount: depositA,
      depositUpToLimit: false,
    });

    await userProgram.provider.sendAndConfirm(new Transaction().add(depositAIx));

    const oracleMeta = composeRemainingAccounts([
      [bankUsdc, oracles.usdcOracle.publicKey],
      [bankA, oracles.tokenAOracle.publicKey],
      [bankSol, oracles.wsolOracle.publicKey],
    ]);

    const borrowUsdcIx = await borrowIx(user.mrgnProgram, {
      marginfiAccount: userMarginfiAccount,
      bank: bankUsdc,
      amount: borrowUsdc,
      tokenAccount: user.usdcAccount,
      remaining: oracleMeta,
    });

    await userProgram.provider.sendAndConfirm(new Transaction().add(borrowUsdcIx));

    // Set emissions destination to the authority before placing any orders
    const setEmissionsDestIx = await updateEmissionsDestination(user.mrgnProgram, {
      marginfiAccount: userMarginfiAccount,
      destinationAccount: user.wallet.publicKey,
    });
    await userProgram.provider.sendAndConfirm(new Transaction().add(setEmissionsDestIx));

  });

  after(async () => {
    // Revert the changes after the tests
    await restoreOracles();
  });

  describe("order placement", () => {
    it("places an order with one asset/one liability - happy path", async () => {
      const bankKeys = [bankA, bankUsdc];
      const trigger: OrderTriggerArgs = {
        stopLoss: { threshold: stopLossThreshold, maxSlippage },
      };

      const ix = await placeOrderIx(program, {
        marginfiAccount: userMarginfiAccount,
        authority: user.wallet.publicKey,
        feePayer: user.wallet.publicKey,
        bankKeys,
        trigger,
      });

      await userProgram.provider.sendAndConfirm(new Transaction().add(ix));

      const [orderPk] = deriveOrderPda(
        program.programId,
        userMarginfiAccount,
        bankKeys
      );
      const orderAccount = await program.account.order.fetch(orderPk);
      const userAccount = await program.account.marginfiAccount.fetch(
        userMarginfiAccount
      );

      expect(orderAccount.marginfiAccount.toBase58()).to.equal(
        userMarginfiAccount.toBase58()
      );
      expect(orderAccount.tags.length).to.equal(2);
      assert.isAbove(Number(orderAccount.tags[0]), 0);
      assert.isAbove(Number(orderAccount.tags[1]), 0);

      assert.notDeepEqual(
        Number(orderAccount.tags[0]),
        Number(orderAccount.tags[1])
      );

      const index0 = userAccount.lendingAccount.balances.findIndex(
        (balance) => balance.tag == orderAccount.tags[0]
      );
      const index1 = userAccount.lendingAccount.balances.findIndex(
        (balance) => balance.tag == orderAccount.tags[1]
      );

      assert.notDeepEqual(index0, -1);
      assert.notDeepEqual(index1, -1);
    });

    it("rejects duplicate bank keys - should fail", async () => {
      await expectFailedTxWithError(
        async () => {
          const ix = await placeOrderIx(program, {
            marginfiAccount: userMarginfiAccount,
            authority: user.wallet.publicKey,
            feePayer: user.wallet.publicKey,
            bankKeys: [bankA, bankA],
            trigger: { stopLoss: { threshold: stopLossThreshold, maxSlippage } },
          });

          await userProgram.provider.sendAndConfirm(new Transaction().add(ix));
        },
        "DuplicateBalance",
        6103
      );
    });

    it("rejects when both balances are assets - should fail", async () => {
      await expectFailedTxWithError(
        async () => {
          const ix = await placeOrderIx(program, {
            marginfiAccount: userMarginfiAccount,
            authority: user.wallet.publicKey,
            feePayer: user.wallet.publicKey,
            bankKeys: [bankA, bankSol],
            trigger: { stopLoss: { threshold: stopLossThreshold, maxSlippage } },
          });

          await userProgram.provider.sendAndConfirm(new Transaction().add(ix));
        },
        "InvalidAssetOrLiabilitiesCount",
        6110
      );
    });

    it("rejects creating the same order twice - should fail", async () => {
      const bankKeys = [bankA, bankUsdc];

      await expectFailedTxWithMessage(async () => {
        const ix = await placeOrderIx(program, {
          marginfiAccount: userMarginfiAccount,
          authority: user.wallet.publicKey,
          feePayer: user.wallet.publicKey,
          bankKeys,
          trigger: { both: { stopLoss: stopLossThreshold, takeProfit: takeProfitThreshold, maxSlippage } },
        });

        await userProgram.provider.sendAndConfirm(new Transaction().add(ix));
      }, "already in use");
    });
  });

  describe("order maintenance", () => {
    const placeTestOrder = async (bankKeys: PublicKey[] = [bankA, bankUsdc]) => {
      const ix = await placeOrderIx(program, {
        marginfiAccount: userMarginfiAccount,
        authority: user.wallet.publicKey,
        feePayer: user.wallet.publicKey,
        bankKeys,
        trigger: { stopLoss: { threshold: stopLossThreshold, maxSlippage } },
      });

      await userProgram.provider.sendAndConfirm(new Transaction().add(ix));
      const [orderPk] = deriveOrderPda(program.programId, userMarginfiAccount, bankKeys);
      return orderPk;
    };

    it("closes an order as the authority - happy path", async () => {
      const bankKeys = [bankA, bankUsdc];
      const [orderPk] = deriveOrderPda(program.programId, userMarginfiAccount, bankKeys);

      const ix = await closeOrderIx(program, {
        marginfiAccount: userMarginfiAccount,
        authority: user.wallet.publicKey,
        order: orderPk,
        feeRecipient: user.wallet.publicKey,
      });

      await userProgram.provider.sendAndConfirm(new Transaction().add(ix));

      const closed = await program.provider.connection.getAccountInfo(orderPk);
      expect(closed).to.be.null;
    });

    it("keeper closes an order - happy path", async () => {
      const orderPk = await placeTestOrder();

      const keeper = users[1];
      const keeperProgram = keeper.mrgnProgram as Program<Marginfi>;

      // Clear the liability, so at least one order tag has no active balance
      const repayInstruction = await program.methods
        .lendingAccountRepay(borrowUsdc, true)
        .accountsPartial({
          marginfiAccount: userMarginfiAccount,
          authority: user.wallet.publicKey,
          bank: bankUsdc,
          signerTokenAccount: user.usdcAccount,
          tokenProgram: TOKEN_PROGRAM_ID,
        })
        .instruction();

      // Ensure the user's ATA exists
      const bankAccount = await program.account.bank.fetch(bankUsdc);
      const emissionsMint = bankAccount.emissionsMint;
      const emissionsAta = getAssociatedTokenAddressSync(
        emissionsMint,
        user.wallet.publicKey
      );

      const createEmissionsAtaIx = createAssociatedTokenAccountIdempotentInstruction(
        user.wallet.publicKey,
        emissionsAta,
        user.wallet.publicKey,
        emissionsMint
      );

      // Claim emissions on the liability balance before closing it
      const withdrawEmissionsInstruction = await program.methods
        .lendingAccountWithdrawEmissions()
        .accountsPartial({
          marginfiAccount: userMarginfiAccount,
          authority: user.wallet.publicKey,
          bank: bankUsdc,
          destinationAccount: emissionsAta,
          tokenProgram: TOKEN_PROGRAM_ID,
        })
        .instruction();

      await userProgram.provider.sendAndConfirm(
        new Transaction()
          .add(createEmissionsAtaIx)
          .add(withdrawEmissionsInstruction)
          .add(repayInstruction)
      );

      const ix = await keeperCloseOrderIx(program, {
        marginfiAccount: userMarginfiAccount,
        order: orderPk,
        feeRecipient: keeper.wallet.publicKey,
      });

      await keeperProgram.provider.sendAndConfirm(new Transaction().add(ix));

      const closed = await program.provider.connection.getAccountInfo(orderPk);
      expect(closed).to.be.null;

      // Borrow the USDC again for other tests
      const oracleMeta = composeRemainingAccounts([
        [bankUsdc, oracles.usdcOracle.publicKey],
        [bankA, oracles.tokenAOracle.publicKey],
        [bankSol, oracles.wsolOracle.publicKey],
      ]).map((pubkey) => ({ pubkey, isSigner: false, isWritable: false }));

      const borrowIx = await program.methods
        .lendingAccountBorrow(borrowUsdc)
        .accountsPartial({
          marginfiAccount: userMarginfiAccount,
          authority: user.wallet.publicKey,
          bank: bankUsdc,
          destinationTokenAccount: user.usdcAccount,
          tokenProgram: TOKEN_PROGRAM_ID,
        })
        .remainingAccounts(oracleMeta)
        .instruction();

      await userProgram.provider.sendAndConfirm(new Transaction().add(borrowIx));
    });

    it("keeper close fails when the condition is not satisfied - should fail", async () => {
      const orderPk = await placeTestOrder();
      const keeper = users[1];
      const keeperProgram = keeper.mrgnProgram as Program<Marginfi>;

      await expectFailedTxWithError(
        async () => {
          const ix = await keeperCloseOrderIx(program, {
            marginfiAccount: userMarginfiAccount,
            order: orderPk,
            feeRecipient: keeper.wallet.publicKey,
          });
          await keeperProgram.provider.sendAndConfirm(new Transaction().add(ix));
        },
        "LiquidatorOrderCloseNotAllowed",
        6105
      );
    });

    it("sets liquidator close flags - happy path", async () => {
      const ix = await setLiquidatorCloseFlagsIx(program, {
        marginfiAccount: userMarginfiAccount,
        authority: user.wallet.publicKey,
        bankKeysOpt: [bankA],
      });

      await userProgram.provider.sendAndConfirm(new Transaction().add(ix));

      const acc = await program.account.marginfiAccount.fetch(userMarginfiAccount);
      expect(acc).to.exist;
    });

    it("keeper closes order after setLiquidatorCloseFlags - happy path", async () => {
      const bankKeys = [bankA, bankUsdc];
      const [orderPk] = deriveOrderPda(program.programId, userMarginfiAccount, bankKeys);

      const keeper = users[1];
      const keeperProgram = keeper.mrgnProgram as Program<Marginfi>;

      const ix = await keeperCloseOrderIx(program, {
        marginfiAccount: userMarginfiAccount,
        order: orderPk,
        feeRecipient: keeper.wallet.publicKey,
      });

      await keeperProgram.provider.sendAndConfirm(new Transaction().add(ix));

      const closed = await program.provider.connection.getAccountInfo(orderPk);
      expect(closed).to.be.null;
    });
  });

  describe("order execution", () => {
    const bankKeys = [bankA, bankUsdc];
    let orderPk: PublicKey;
    let userEmissionsAta: PublicKey;
    let keeper: MockUser;
    let keeperProgram: Program<Marginfi>;
    let keeperTokenAata: PublicKey;
    const confFactor = ORACLE_CONF_INTERVAL * CONF_INTERVAL_MULTIPLE;

    const buildRemaining = (includeUsdc = true, includeA = true, includeSol = true) => {
      const pairs: [PublicKey, PublicKey][] = [];

      if (includeUsdc) {
        pairs.push([bankUsdc, oracles.usdcOracle.publicKey]);
      }

      if (includeA) {
        pairs.push([bankA, oracles.tokenAOracle.publicKey]);
      }

      if (includeSol) {
        pairs.push([bankSol, oracles.wsolOracle.publicKey]);
      }

      return composeRemainingAccounts(pairs);
    };

    const fetchPricingInputs = async () => {
      const bankAAccount = await program.account.bank.fetch(bankA);
      const bankUsdcAccount = await program.account.bank.fetch(bankUsdc);
      const accBeforePricing = await program.account.marginfiAccount.fetch(userMarginfiAccount);

      const balA = accBeforePricing.lendingAccount.balances.find((b: any) => b.bankPk && b.bankPk.equals(bankA));
      const balUsdc = accBeforePricing.lendingAccount.balances.find((b: any) => b.bankPk && b.bankPk.equals(bankUsdc));

      const assetShares = wrappedI80F48toBigNumber(balA.assetShares).toNumber();
      const assetShareValue = wrappedI80F48toBigNumber(bankAAccount.assetShareValue).toNumber();
      const assetNative = (assetShares * assetShareValue) / (10 ** bankAAccount.mintDecimals);

      const liabShares = wrappedI80F48toBigNumber(balUsdc.liabilityShares).toNumber();
      const liabShareValue = wrappedI80F48toBigNumber(bankUsdcAccount.liabilityShareValue).toNumber();
      const liabNative = (liabShares * liabShareValue) / (10 ** bankUsdcAccount.mintDecimals);

      return { assetNative, liabNative };
    };

    const computeBiasedPrice = (
      assetNative: number,
      liabNative: number,
      threshold: number,
      confFactor: number,
      offset: number
    ) => {
      const biasedLiabValue = liabNative * (oracles.usdcPrice * (1 + confFactor));
      const targetBiased = (threshold + offset) + biasedLiabValue;
      const basePriceNeeded = targetBiased / assetNative;
      return basePriceNeeded / (1 - confFactor);
    };

    const calcWithdrawAmount = (assetPrice: number) => {
      const liabilityAmountFloat = Number(borrowUsdc) / 10 ** ecosystem.usdcDecimals;
      const liabilityValue = liabilityAmountFloat * oracles.usdcPrice;
      const assetAmountFloat = liabilityValue / assetPrice;
      const assetAmountUnits = Math.ceil(assetAmountFloat * 10 ** ecosystem.tokenADecimals);
      return new BN(assetAmountUnits);
    };

    const buildExecutionIxs = async (startRemaining: PublicKey[], endRemaining: PublicKey[], withdrawAmount: BN) => {
      const [executeRecordPk] = deriveExecuteOrderPda(program.programId, orderPk);

      const startIx = await startExecuteOrderIx(program, {
        group: marginfiGroup.publicKey,
        marginfiAccount: userMarginfiAccount,
        feePayer: keeper.wallet.publicKey,
        executor: keeper.wallet.publicKey,
        order: orderPk,
        remaining: startRemaining,
      });

      const withdrawEmissionsPermIx = await program.methods
        .lendingAccountWithdrawEmissionsPermissionless()
        .accountsPartial({
          marginfiAccount: userMarginfiAccount,
          bank: bankUsdc,
          destinationAccount: userEmissionsAta,
          tokenProgram: TOKEN_PROGRAM_ID,
        })
        .instruction();

      const repayInstruction = await program.methods
        .lendingAccountRepay(borrowUsdc, true)
        .accountsPartial({
          marginfiAccount: userMarginfiAccount,
          authority: keeper.wallet.publicKey,
          bank: bankUsdc,
          signerTokenAccount: keeper.usdcAccount,
          tokenProgram: TOKEN_PROGRAM_ID,
        })
        .instruction();

      const withdrawRemaining = composeRemainingAccounts([
        [bankA, oracles.tokenAOracle.publicKey],
      ]).map((pubkey) => ({ pubkey, isSigner: false, isWritable: false }));

      const withdrawInstruction = await program.methods
        .lendingAccountWithdraw(withdrawAmount, false)
        .accountsPartial({
          marginfiAccount: userMarginfiAccount,
          authority: keeper.wallet.publicKey,
          bank: bankA,
          destinationTokenAccount: keeperTokenAata,
          tokenProgram: TOKEN_PROGRAM_ID,
        })
        .remainingAccounts(withdrawRemaining)
        .instruction();

      const endIx = await endExecuteOrderIx(program, {
        group: marginfiGroup.publicKey,
        marginfiAccount: userMarginfiAccount,
        executor: keeper.wallet.publicKey,
        order: orderPk,
        executeRecord: executeRecordPk,
        feeRecipient: keeper.wallet.publicKey,
        remaining: endRemaining,
      });

      return { startIx, withdrawEmissionsPermIx, repayInstruction, withdrawInstruction, endIx };
    };

    before(async () => {

      const ixPlace = await placeOrderIx(program, {
        marginfiAccount: userMarginfiAccount,
        authority: user.wallet.publicKey,
        feePayer: user.wallet.publicKey,
        bankKeys,
        trigger: { takeProfit: { threshold: highTakeProfit, maxSlippage } },
      });

      await userProgram.provider.sendAndConfirm(new Transaction().add(ixPlace));

      [orderPk] = deriveOrderPda(program.programId, userMarginfiAccount, bankKeys);

      // Ensure emissions destination is registered for the user
      const bankAccount = await program.account.bank.fetch(bankUsdc);
      const emissionsMint = bankAccount.emissionsMint;
      userEmissionsAta = getAssociatedTokenAddressSync(emissionsMint, user.wallet.publicKey);

      const ensureUserEmissionsAtaIx = createAssociatedTokenAccountIdempotentInstruction(
        user.wallet.publicKey,
        userEmissionsAta,
        user.wallet.publicKey,
        emissionsMint
      );

      const updateEmissionsIx = await program.methods
        .marginfiAccountUpdateEmissionsDestinationAccount()
        .accountsPartial({
          marginfiAccount: userMarginfiAccount,
          authority: user.wallet.publicKey,
          destinationAccount: user.wallet.publicKey,
        })
        .instruction();
      await userProgram.provider.sendAndConfirm(
        new Transaction().add(ensureUserEmissionsAtaIx).add(updateEmissionsIx)
      );

      // Fund keeper with USDC so they can repay during execution
      keeper = users[1];
      keeperProgram = keeper.mrgnProgram as Program<Marginfi>;
      keeperMarginfiAccount = keeper.accounts.get(USER_ACCOUNT);

      const mintUsdcToKeeperTx = new Transaction().add(
        createMintToInstruction(
          ecosystem.usdcMint.publicKey,
          keeper.usdcAccount,
          wallet.publicKey,
          Number(borrowUsdc) * 10
        )
      );
      await program.provider.sendAndConfirm(mintUsdcToKeeperTx, [wallet.payer]);

      keeperTokenAata = getAssociatedTokenAddressSync(
        ecosystem.tokenAMint.publicKey,
        keeper.wallet.publicKey
      );

      const ensureKeeperAta = createAssociatedTokenAccountIdempotentInstruction(
        keeper.wallet.publicKey,
        keeperTokenAata,
        keeper.wallet.publicKey,
        ecosystem.tokenAMint.publicKey
      );
      await keeperProgram.provider.sendAndConfirm(new Transaction().add(ensureKeeperAta));
    });

    it("fails when trigger not yet reached - should fail", async () => {
      const { assetNative, liabNative } = await fetchPricingInputs();
      const threshold = wrappedI80F48toBigNumber(highTakeProfit).toNumber();
      const biasedPrice = computeBiasedPrice(assetNative, liabNative, threshold, confFactor, -1);

      oracles.tokenAPrice = biasedPrice;
      const slot = new BN(Math.floor(Date.now() / 1000));
      await refreshPullOracles(oracles, wallet.payer, slot, Math.floor(Date.now() / 1000));

      const remaining = buildRemaining();
      const withdrawAmount = calcWithdrawAmount(oracles.tokenAPrice);
      const {
        startIx,
        withdrawEmissionsPermIx,
        repayInstruction,
        withdrawInstruction,
        endIx,
      } = await buildExecutionIxs(remaining, remaining, withdrawAmount);

      await expectFailedTxWithError(
        async () => {
          await keeperProgram.provider.sendAndConfirm(
            new Transaction()
              .add(startIx)
              .add(withdrawEmissionsPermIx)
              .add(repayInstruction)
              .add(withdrawInstruction)
              .add(endIx)
          );
        },
        "OrderTriggerNotMet",
        6107
      );

      oracles.tokenAPrice = 10; // Reset price for other tests
      await refreshPullOracles(oracles, wallet.payer, slot, Math.floor(Date.now() / 1000));

    });

    it("fails when touching uninvolved balance - should fail", async () => {
      const { assetNative, liabNative } = await fetchPricingInputs();
      const threshold = wrappedI80F48toBigNumber(highTakeProfit).toNumber();
      const biasedPrice = computeBiasedPrice(assetNative, liabNative, threshold, confFactor, 1);

      // Place price above trigger
      oracles.tokenAPrice = biasedPrice;
      const slot = new BN(Math.floor(Date.now() / 1000));
      await refreshPullOracles(oracles, wallet.payer, slot, Math.floor(Date.now() / 1000));

      const startRemaining = buildRemaining();
      const endRemaining = buildRemaining(false, true, true);
      const withdrawAmount = calcWithdrawAmount(oracles.tokenAPrice);
      const {
        startIx,
        withdrawEmissionsPermIx,
        repayInstruction,
        withdrawInstruction,
        endIx,
      } = await buildExecutionIxs(startRemaining, endRemaining, withdrawAmount);

      // Ensure the keeper has a wSOL ATA
      const keeperWsolAta = getAssociatedTokenAddressSync(ecosystem.wsolMint.publicKey, keeper.wallet.publicKey);
      const ensureKeeperWsolAtaIx = createAssociatedTokenAccountIdempotentInstruction(
        keeper.wallet.publicKey,
        keeperWsolAta,
        keeper.wallet.publicKey,
        ecosystem.wsolMint.publicKey
      );

      await keeperProgram.provider.sendAndConfirm(new Transaction().add(ensureKeeperWsolAtaIx));
      keeper.wsolAccount = keeperWsolAta;

      const withdrawSolRemaining = composeRemainingAccounts([
        [bankSol, oracles.wsolOracle.publicKey],
      ]).map((pubkey) => ({ pubkey, isSigner: false, isWritable: false }));

      const withdrawSol = await program.methods
        .lendingAccountWithdraw(new BN(1000), false)
        .accountsPartial({
          marginfiAccount: userMarginfiAccount,
          authority: keeper.wallet.publicKey,
          bank: bankSol,
          destinationTokenAccount: keeper.wsolAccount,
          tokenProgram: TOKEN_PROGRAM_ID,
        })
        .remainingAccounts(withdrawSolRemaining)
        .instruction();

      await expectFailedTxWithError(
        async () => {
          await keeperProgram.provider.sendAndConfirm(
            new Transaction()
              .add(startIx)
              .add(withdrawEmissionsPermIx)
              .add(repayInstruction)
              .add(withdrawInstruction)
              .add(withdrawSol) // This touches an uninvolved balance
              .add(endIx)
          );
        },
        "IllegalBalanceState",
        6040
      );

      oracles.tokenAPrice = 10; // Reset price
      await refreshPullOracles(oracles, wallet.payer, slot, Math.floor(Date.now() / 1000));
    });

    it("Take-profit!!! - happy path", async () => {
      const { assetNative, liabNative } = await fetchPricingInputs();
      const threshold = wrappedI80F48toBigNumber(highTakeProfit).toNumber();
      const biasedPrice = computeBiasedPrice(assetNative, liabNative, threshold, confFactor, 1);

      oracles.tokenAPrice = biasedPrice;
      const slot = new BN(Math.floor(Date.now() / 1000));
      await refreshPullOracles(oracles, wallet.payer, slot, Math.floor(Date.now() / 1000));

      const orderBefore = await program.account.order.fetch(orderPk);
      const accBefore = await program.account.marginfiAccount.fetch(userMarginfiAccount);

      const startRemaining = buildRemaining();
      const endRemaining = buildRemaining();
      const withdrawAmount = calcWithdrawAmount(oracles.tokenAPrice);
      const {
        startIx,
        withdrawEmissionsPermIx,
        repayInstruction,
        withdrawInstruction,
        endIx,
      } = await buildExecutionIxs(startRemaining, endRemaining, withdrawAmount);

      await keeperProgram.provider.sendAndConfirm(
        new Transaction()
          .add(startIx)
          .add(withdrawEmissionsPermIx)
          .add(repayInstruction)
          .add(withdrawInstruction)
          .add(endIx)
      );

      // Verify the order account has been closed
      const orderInfo = await program.provider.connection.getAccountInfo(orderPk);
      assert.isNull(orderInfo, "expected order account to be closed after execution");

      // Fetch post-execution marginfi account
      const accAfter = await program.account.marginfiAccount.fetch(userMarginfiAccount);

      // Determine bank PKs for the asset and liability balances from pre-exec state
      const assetTag = orderBefore.tags[0];
      const liabilityTag = orderBefore.tags[1];
      const preAsset = accBefore.lendingAccount.balances.find((b: any) => Number(b.tag) === Number(assetTag));
      const preLiability = accBefore.lendingAccount.balances.find((b: any) => Number(b.tag) === Number(liabilityTag));

      const assetBankPk = preAsset.bankPk.toString();
      const liabilityBankPk = preLiability.bankPk.toString();

      // Asset should still exist
      const postAsset = accAfter.lendingAccount.balances.find((b: any) => b.bankPk && b.bankPk.toString() === assetBankPk);
      assert.exists(postAsset, `expected asset balance for bank ${assetBankPk} to still exist after execution`);

      // Liability should no longer exist
      const postLiability = accAfter.lendingAccount.balances.find((b: any) => b.bankPk && b.bankPk.toString() === liabilityBankPk);
      assert.isUndefined(postLiability, `expected liability balance for bank ${liabilityBankPk} to be removed after execution`);

      // Balances not part of the order must remain unchanged
      const orderBankSet = new Set([assetBankPk, liabilityBankPk]);
      for (const preBal of accBefore.lendingAccount.balances) {
        const preBank = preBal.bankPk.toString();
        if (orderBankSet.has(preBank)) continue;

        const postBal = accAfter.lendingAccount.balances.find((b: any) => b.bankPk && b.bankPk.toString() === preBank);
        assert.exists(postBal, `expected other balance for bank ${preBank} to still exist after execution`);
        assert.deepEqual(preBal, postBal, `pre balance to equal post balance ${preBank}`);
      }

      // Compute asset-value estimate and assert it exceeds the trigger threshold.
      // We don't take advantage of the max-fee or slippage here, so we compare it 
      // directly to the threshold.
      const singleAssetPrice = oracles.tokenAPrice;
      const singleAssetDecimals = ecosystem.tokenADecimals;

      const assetSharesPost = wrappedI80F48toBigNumber(postAsset.assetShares).toNumber();
      const assetNativeAmount = assetSharesPost / 10 ** singleAssetDecimals;
      const assetValue = assetNativeAmount * singleAssetPrice;

      assert.isAbove(assetValue, wrappedI80F48toBigNumber(highTakeProfit).toNumber(), `expected asset value (${assetValue}) to exceed 800`);
    });
  });
});
