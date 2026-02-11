import { BN, Program } from "@coral-xyz/anchor";
import {
  PublicKey,
  Keypair,
  Transaction,
  SystemProgram,
  SYSVAR_RENT_PUBKEY,
} from "@solana/web3.js";
import {
  bankrunContext,
  bankRunProvider,
  banksClient,
  ecosystem,
  globalProgramAdmin,
  groupAdmin,
  kaminoAccounts,
  klendBankrunProgram,
  MARKET,
  oracles,
  TOKEN_A_RESERVE,
  KAMINO_TOKEN_A_BANK,
  users,
  verbose,
  bankrunProgram,
  A_FARM_STATE,
  A_FARM_VAULTS_AUTHORITY,
  A_OBLIGATION_USER_STATE,
  A_REWARD_MINT,
  A_REWARD_TREASURY_VAULT,
  A_REWARD_VAULT,
  A_TREASURY_VAULTS_AUTHORITY as TREASURY_VAULTS_AUTHORITY,
  farmAccounts,
  GLOBAL_CONFIG,
  USDC_RESERVE,
  KAMINO_USDC_BANK,
  globalFeeWallet,
} from "./rootHooks";
import { processBankrunTransaction } from "./utils/tools";
import { assert } from "chai";
import {
  TOKEN_PROGRAM_ID,
  createMintToInstruction,
  getAssociatedTokenAddressSync,
  createAssociatedTokenAccountIdempotentInstruction,
} from "@solana/spl-token";
import {
  assertBankrunTxFailed,
  assertBNEqual,
  assertKeysEqual,
  getTokenBalance,
} from "./utils/genericTests";
import { Clock, ProgramTestContext } from "solana-bankrun";
import { FARMS_PROGRAM_ID, KLEND_PROGRAM_ID } from "./utils/types";
import { Farms } from "./fixtures/kamino_farms";
import farmsIdl from "../idls/kamino_farms.json";
import { USER_ACCOUNT_K } from "./utils/mocks";
import { lendingMarketAuthPda } from "@kamino-finance/klend-sdk";
import {
  deriveLiquidityVaultAuthority,
  deriveBaseObligation,
  deriveUserState,
  deriveGlobalFeeState,
} from "./utils/pdas";
import {
  makeKaminoDepositIx,
  makeKaminoHarvestRewardIx,
  makeKaminoWithdrawIx,
} from "./utils/kamino-instructions";
import {
  simpleRefreshReserve,
  simpleRefreshObligation,
} from "./utils/kamino-utils";
import { composeRemainingAccounts } from "./utils/user-instructions";

let ctx: ProgramTestContext;
let kfarmsBankrunProgram: Program<Farms>;

const REWARD_AMOUNT = 1_000_000;

const CONFIG_SIZE = 2136;
const FARM_SIZE = 8336;

describe("k13: Kamino Farms Harvest Reward", () => {
  before(async () => {
    ctx = bankrunContext;

    kfarmsBankrunProgram = new Program<Farms>(
      farmsIdl as Farms,
      bankRunProvider
    );
  });

  it("Initializes global config for farms", async () => {
    const globalConfig = Keypair.generate();

    const [treasuryVaultsAuthority] = PublicKey.findProgramAddressSync(
      [Buffer.from("authority"), globalConfig.publicKey.toBuffer()],
      FARMS_PROGRAM_ID
    );

    const tx = new Transaction().add(
      SystemProgram.createAccount({
        fromPubkey: groupAdmin.wallet.publicKey,
        newAccountPubkey: globalConfig.publicKey,
        space: 8 + CONFIG_SIZE,
        lamports:
          await bankRunProvider.connection.getMinimumBalanceForRentExemption(
            8 + CONFIG_SIZE
          ),
        programId: FARMS_PROGRAM_ID,
      }),
      await kfarmsBankrunProgram.methods
        .initializeGlobalConfig()
        .accounts({
          globalAdmin: groupAdmin.wallet.publicKey,
          globalConfig: globalConfig.publicKey,
          treasuryVaultsAuthority: treasuryVaultsAuthority,
          systemProgram: SystemProgram.programId,
        })
        .instruction()
    );

    await processBankrunTransaction(
      ctx,
      tx,
      [groupAdmin.wallet, globalConfig],
      false,
      true
    );
    farmAccounts.set(GLOBAL_CONFIG, globalConfig.publicKey);
    farmAccounts.set(TREASURY_VAULTS_AUTHORITY, treasuryVaultsAuthority);

    if (verbose) {
      console.log("Global config: " + globalConfig.publicKey);
    }
  });

  it("Initializes a farm via klend - initFarmsForReserve", async () => {
    const farmState = Keypair.generate();
    const globalConfig = farmAccounts.get(GLOBAL_CONFIG);
    const rewardMint = ecosystem.tokenAMint.publicKey;
    farmAccounts.set(A_REWARD_MINT, rewardMint);

    const tokenAReserve = kaminoAccounts.get(TOKEN_A_RESERVE);
    const market = kaminoAccounts.get(MARKET);

    const [lendingMarketAuthority] = PublicKey.findProgramAddressSync(
      [Buffer.from("lma"), market.toBuffer()],
      KLEND_PROGRAM_ID
    );

    const [farmVaultsAuthority] = PublicKey.findProgramAddressSync(
      [Buffer.from("authority"), farmState.publicKey.toBuffer()],
      FARMS_PROGRAM_ID
    );

    const tx = new Transaction().add(
      SystemProgram.createAccount({
        fromPubkey: groupAdmin.wallet.publicKey,
        newAccountPubkey: farmState.publicKey,
        space: 8 + FARM_SIZE,
        lamports:
          await bankRunProvider.connection.getMinimumBalanceForRentExemption(
            8 + FARM_SIZE
          ),
        programId: FARMS_PROGRAM_ID,
      }),
      await klendBankrunProgram.methods
        .initFarmsForReserve(0) // mode 0 = collateral
        .accounts({
          lendingMarketOwner: groupAdmin.wallet.publicKey,
          lendingMarket: market,
          lendingMarketAuthority,
          reserve: tokenAReserve,
          farmsProgram: FARMS_PROGRAM_ID,
          farmsGlobalConfig: globalConfig,
          farmState: farmState.publicKey,
          farmsVaultAuthority: farmVaultsAuthority,
          rent: SYSVAR_RENT_PUBKEY,
          systemProgram: SystemProgram.programId,
        })
        .instruction()
    );

    await processBankrunTransaction(
      ctx,
      tx,
      [groupAdmin.wallet, farmState],
      false,
      true
    );
    farmAccounts.set(A_FARM_STATE, farmState.publicKey);
    farmAccounts.set(A_FARM_VAULTS_AUTHORITY, farmVaultsAuthority);

    const reserveAccount = await klendBankrunProgram.account.reserve.fetch(
      tokenAReserve
    );
    assertKeysEqual(reserveAccount.farmCollateral, farmState.publicKey);

    if (verbose) {
      console.log("Farm state: " + farmState.publicKey);
    }
  });

  it("(admin) Initialize reward for farm", async () => {
    const farmState = farmAccounts.get(A_FARM_STATE);
    const globalConfig = farmAccounts.get(GLOBAL_CONFIG);
    const rewardMint = farmAccounts.get(A_REWARD_MINT);
    const farmVaultsAuthority = farmAccounts.get(A_FARM_VAULTS_AUTHORITY);
    const treasuryVaultsAuthority = farmAccounts.get(TREASURY_VAULTS_AUTHORITY);

    // Derive reward vault and treasury vault PDAs
    const [rewardVault] = PublicKey.findProgramAddressSync(
      [Buffer.from("rvault"), farmState.toBuffer(), rewardMint.toBuffer()],
      FARMS_PROGRAM_ID
    );

    const [rewardTreasuryVault] = PublicKey.findProgramAddressSync(
      [Buffer.from("tvault"), globalConfig.toBuffer(), rewardMint.toBuffer()],
      FARMS_PROGRAM_ID
    );

    // Extra initialization needed for rewards to accrue over time
    const rewardPoints = [
      {
        tsStart: 0,
        rewardPerTimeUnit: 1_000_000,
      },
    ];

    // Manual serialization based on Kamino farm SDK approach
    function serializeRewardCurvePoint(
      reward_index: number,
      points: { tsStart: number; rewardPerTimeUnit: number }[]
    ): Uint8Array {
      const buffer = Buffer.alloc(8 + 4 + 16 * points.length); // u64 reward_index + u32 length + points
      buffer.writeBigUint64LE(BigInt(reward_index), 0); // reward_index
      buffer.writeUInt32LE(points.length, 8); // Vec length prefix
      for (let i = 0; i < points.length; i++) {
        buffer.writeBigUint64LE(BigInt(points[i].tsStart), 12 + 16 * i);
        buffer.writeBigUint64LE(
          BigInt(points[i].rewardPerTimeUnit),
          20 + 16 * i
        );
      }
      return Uint8Array.from(buffer);
    }

    const tx = new Transaction().add(
      await kfarmsBankrunProgram.methods
        .initializeReward()
        .accounts({
          farmAdmin: groupAdmin.wallet.publicKey,
          farmState,
          globalConfig,
          rewardMint,
          rewardVault,
          rewardTreasuryVault,
          farmVaultsAuthority,
          treasuryVaultsAuthority,
          tokenProgram: TOKEN_PROGRAM_ID,
          systemProgram: SystemProgram.programId,
          rent: SYSVAR_RENT_PUBKEY,
        })
        .instruction(),
      await kfarmsBankrunProgram.methods
        .updateFarmConfig(
          16, // mode 16 = UpdateRewardScheduleCurvePoints in FarmConfigOption enum
          Buffer.from(serializeRewardCurvePoint(0, rewardPoints))
        )
        .accounts({
          signer: groupAdmin.wallet.publicKey,
          farmState,
          scopePrices: null,
        })
        .instruction()
    );

    await processBankrunTransaction(ctx, tx, [groupAdmin.wallet], false, true);
    farmAccounts.set(A_REWARD_VAULT, rewardVault);
    farmAccounts.set(A_REWARD_TREASURY_VAULT, rewardTreasuryVault);

    if (verbose) {
      console.log("Reward vault: " + rewardVault);
      console.log("Reward treasury vault: " + rewardTreasuryVault);
    }
  });

  it("(admin) Add rewards to farm", async () => {
    const farmState = farmAccounts.get(A_FARM_STATE);
    const rewardMint = farmAccounts.get(A_REWARD_MINT);
    const rewardVault = farmAccounts.get(A_REWARD_VAULT);
    const farmVaultsAuthority = farmAccounts.get(A_FARM_VAULTS_AUTHORITY);

    const adminRewardAta = getAssociatedTokenAddressSync(
      rewardMint,
      groupAdmin.wallet.publicKey
    );

    const tx = new Transaction().add(
      createAssociatedTokenAccountIdempotentInstruction(
        groupAdmin.wallet.publicKey,
        adminRewardAta,
        groupAdmin.wallet.publicKey,
        rewardMint
      ),
      createMintToInstruction(
        rewardMint,
        adminRewardAta,
        globalProgramAdmin.wallet.publicKey,
        REWARD_AMOUNT * 10 ** ecosystem.tokenADecimals
      ),
      await kfarmsBankrunProgram.methods
        .addRewards(
          new BN(REWARD_AMOUNT * 10 ** ecosystem.tokenADecimals),
          new BN(0)
        )
        .accounts({
          payer: groupAdmin.wallet.publicKey,
          farmState,
          rewardMint,
          rewardVault,
          farmVaultsAuthority,
          payerRewardTokenAta: adminRewardAta,
          scopePrices: null,
          tokenProgram: TOKEN_PROGRAM_ID,
        })
        .instruction(),
      await kfarmsBankrunProgram.methods
        .refreshFarm()
        .accounts({
          farmState,
          scopePrices: null,
        })
        .instruction()
    );

    await processBankrunTransaction(
      ctx,
      tx,
      [groupAdmin.wallet, globalProgramAdmin.wallet],
      false,
      true
    );

    const rewardVaultBalance = await getTokenBalance(
      bankRunProvider,
      rewardVault
    );
    assert.equal(
      rewardVaultBalance,
      REWARD_AMOUNT * 10 ** ecosystem.tokenADecimals,
      "Reward vault should contain the added rewards"
    );

    if (verbose) {
      console.log("Reward vault balance: " + rewardVaultBalance);
    }
  });

  it("(klend) Initialize obligation farms for Token A reserve", async () => {
    const farmState = farmAccounts.get(A_FARM_STATE);
    const market = kaminoAccounts.get(MARKET);
    const tokenAReserve = kaminoAccounts.get(TOKEN_A_RESERVE);
    const tokenABank = kaminoAccounts.get(KAMINO_TOKEN_A_BANK);

    const [lendingVaultAuthority] = deriveLiquidityVaultAuthority(
      bankrunProgram.programId,
      tokenABank
    );
    const [obligation] = deriveBaseObligation(lendingVaultAuthority, market);
    const [userState] = deriveUserState(
      FARMS_PROGRAM_ID,
      farmState,
      obligation
    );

    const [lendingMarketAuthority] = lendingMarketAuthPda(
      market,
      klendBankrunProgram.programId
    );

    const tx = new Transaction().add(
      await klendBankrunProgram.methods
        .initObligationFarmsForReserve(0) // mode 0 = collateral
        .accounts({
          payer: groupAdmin.wallet.publicKey,
          owner: lendingVaultAuthority,
          obligation,
          lendingMarketAuthority,
          reserve: tokenAReserve,
          reserveFarmState: farmState,
          obligationFarm: userState,
          lendingMarket: market,
          farmsProgram: FARMS_PROGRAM_ID,
          rent: SYSVAR_RENT_PUBKEY,
          systemProgram: SystemProgram.programId,
        })
        .instruction()
    );

    await processBankrunTransaction(ctx, tx, [groupAdmin.wallet], false, true);

    farmAccounts.set(A_OBLIGATION_USER_STATE, userState);

    // Verify the user state was created correctly
    const userStateAccount = await kfarmsBankrunProgram.account.userState.fetch(
      userState
    );
    assertKeysEqual(userStateAccount.farmState, farmState);
    assertKeysEqual(userStateAccount.delegatee, obligation);

    if (verbose) {
      console.log("Initialized user state for obligation: " + obligation);
      console.log("User state: " + userState);
    }
  });

  it("(user 0) Make a deposit to accrue rewards", async () => {
    const user = users[0];
    const depositAmount = new BN(1000 * 10 ** ecosystem.tokenADecimals); // 1000 Token A
    const marginfiAccount = user.accounts.get(USER_ACCOUNT_K);
    const market = kaminoAccounts.get(MARKET);
    const tokenAReserve = kaminoAccounts.get(TOKEN_A_RESERVE);
    const tokenABank = kaminoAccounts.get(KAMINO_TOKEN_A_BANK);
    const farmState = farmAccounts.get(A_FARM_STATE);
    const userState = farmAccounts.get(A_OBLIGATION_USER_STATE);
    console.log("farm state actual: " + farmState + " user " + userState);

    const userTokenABefore = await getTokenBalance(
      bankRunProvider,
      user.tokenAAccount
    );

    const [lendingVaultAuthority] = deriveLiquidityVaultAuthority(
      bankrunProgram.programId,
      tokenABank
    );
    const [obligation] = deriveBaseObligation(lendingVaultAuthority, market);

    const tx = new Transaction().add(
      await simpleRefreshReserve(
        klendBankrunProgram,
        tokenAReserve,
        market,
        oracles.tokenAOracle.publicKey
      ),
      await simpleRefreshObligation(klendBankrunProgram, market, obligation, [
        tokenAReserve,
      ]),
      await makeKaminoDepositIx(
        user.mrgnBankrunProgram,
        {
          marginfiAccount,
          bank: tokenABank,
          signerTokenAccount: user.tokenAAccount,
          lendingMarket: market,
          reserveLiquidityMint: ecosystem.tokenAMint.publicKey,
          obligationFarmUserState: userState,
          reserveFarmState: farmState,
        },
        depositAmount
      ),
      await kfarmsBankrunProgram.methods
        .refreshUserState()
        .accounts({
          userState,
          farmState,
          scopePrices: null,
        })
        .instruction()
    );

    await processBankrunTransaction(ctx, tx, [user.wallet], false, true);

    const userTokenAAfter = await getTokenBalance(
      bankRunProvider,
      user.tokenAAccount
    );
    const deposited = userTokenABefore - userTokenAAfter;

    assert.equal(
      deposited,
      depositAmount.toNumber(),
      "Should have deposited the correct amount"
    );

    const userStateAccount = await kfarmsBankrunProgram.account.userState.fetch(
      userState
    );
    const obligationAccount =
      await klendBankrunProgram.account.obligation.fetch(obligation);

    const userStateStake = userStateAccount.activeStakeScaled;
    const obligationDeposit = obligationAccount.deposits[0];

    assertBNEqual(userStateStake, obligationDeposit.depositedAmount);

    const clockBefore = await banksClient.getClock();
    console.log(
      `Clock before - Slot: ${clockBefore.slot}, Timestamp: ${clockBefore.unixTimestamp}`
    );
    ctx.setClock(
      new Clock(
        clockBefore.slot,
        clockBefore.epochStartTimestamp,
        clockBefore.epoch,
        clockBefore.leaderScheduleEpoch,
        clockBefore.unixTimestamp + BigInt(1 * 60 * 60)
      )
    );
    const clockAfter = await banksClient.getClock();
    console.log(
      `Time jump: ${clockAfter.unixTimestamp - clockBefore.unixTimestamp
      } seconds`
    );
  });

  it("(user 0) Make a withdraw to test farm interaction", async () => {
    const user = users[0];
    const withdrawAmount = new BN(500 * 10 ** ecosystem.tokenADecimals); // 500 Token A
    const marginfiAccount = user.accounts.get(USER_ACCOUNT_K);
    const market = kaminoAccounts.get(MARKET);
    const tokenAReserve = kaminoAccounts.get(TOKEN_A_RESERVE);
    const tokenABank = kaminoAccounts.get(KAMINO_TOKEN_A_BANK);
    const farmState = farmAccounts.get(A_FARM_STATE);
    const userState = farmAccounts.get(A_OBLIGATION_USER_STATE);
    const usdcReserve = kaminoAccounts.get(USDC_RESERVE);
    const usdcBank = kaminoAccounts.get(KAMINO_USDC_BANK);

    const userTokenABefore = await getTokenBalance(
      bankRunProvider,
      user.tokenAAccount
    );

    const [lendingVaultAuthority] = deriveLiquidityVaultAuthority(
      bankrunProgram.programId,
      tokenABank
    );
    const [obligation] = deriveBaseObligation(lendingVaultAuthority, market);

    const tx = new Transaction().add(
      await simpleRefreshReserve(
        klendBankrunProgram,
        tokenAReserve,
        market,
        oracles.tokenAOracle.publicKey
      ),
      await simpleRefreshObligation(klendBankrunProgram, market, obligation, [
        tokenAReserve,
      ]),
      await makeKaminoWithdrawIx(
        user.mrgnBankrunProgram,
        {
          marginfiAccount,
          authority: user.wallet.publicKey,
          bank: tokenABank,
          destinationTokenAccount: user.tokenAAccount,
          lendingMarket: market,
          reserveLiquidityMint: ecosystem.tokenAMint.publicKey,
          obligationFarmUserState: userState,
          reserveFarmState: farmState,
        },
        {
          amount: withdrawAmount,
          isFinalWithdrawal: false,
          remaining: composeRemainingAccounts([
            [usdcBank, oracles.usdcOracle.publicKey, usdcReserve],
            [tokenABank, oracles.tokenAOracle.publicKey, tokenAReserve],
          ]),

          // remaining: [
          //   usdcBank,
          //   oracles.usdcOracle.publicKey,
          //   usdcReserve,
          //   tokenABank,
          //   oracles.tokenAOracle.publicKey,
          //   tokenAReserve,
          // ],
        }
      )
    );

    await processBankrunTransaction(ctx, tx, [user.wallet], false, true);

    const userTokenAAfter = await getTokenBalance(
      bankRunProvider,
      user.tokenAAccount
    );
    const withdrawn = userTokenAAfter - userTokenABefore;

    assert.isAtLeast(
      withdrawn,
      withdrawAmount.toNumber(),
      "Should have withdrawn at least the requested amount (plus interest)"
    );

    const userStateAccount = await kfarmsBankrunProgram.account.userState.fetch(
      userState
    );
    const obligationAccount =
      await klendBankrunProgram.account.obligation.fetch(obligation);

    const userStateStake = userStateAccount.activeStakeScaled;
    const obligationDeposit = obligationAccount.deposits[0];

    assert.isTrue(
      userStateStake.eq(obligationDeposit.depositedAmount),
      "Farm user state active stake should match obligation deposit amount after withdrawal"
    );

    if (verbose) {
      console.log("Withdrawn amount: " + withdrawn);
      console.log("Remaining stake: " + userStateStake.toString());
    }
  });

  it("(negative test) Should fail to harvest with wrong destination token account owner", async () => {
    const tokenABank = kaminoAccounts.get(KAMINO_TOKEN_A_BANK);

    const farmState = farmAccounts.get(A_FARM_STATE);
    const globalConfig = farmAccounts.get(GLOBAL_CONFIG);
    const rewardMint = farmAccounts.get(A_REWARD_MINT);
    const userState = farmAccounts.get(A_OBLIGATION_USER_STATE);
    const rewardVault = farmAccounts.get(A_REWARD_VAULT);
    const rewardTreasuryVault = farmAccounts.get(A_REWARD_TREASURY_VAULT);
    const farmVaultsAuthority = farmAccounts.get(A_FARM_VAULTS_AUTHORITY);

    const [liquidityVaultAuthority] = deriveLiquidityVaultAuthority(
      bankrunProgram.programId,
      tokenABank
    );

    const [feeState] = deriveGlobalFeeState(bankrunProgram.programId);

    const userRewardAta = getAssociatedTokenAddressSync(
      rewardMint,
      liquidityVaultAuthority,
      true
    );

    // Try to use a destination ATA owned by groupAdmin instead of globalProgramAdmin
    const wrongDestinationAta = getAssociatedTokenAddressSync(
      rewardMint,
      groupAdmin.wallet.publicKey // Wrong owner - should be globalProgramAdmin
    );

    const tx = new Transaction().add(
      createAssociatedTokenAccountIdempotentInstruction(
        groupAdmin.wallet.publicKey,
        userRewardAta,
        liquidityVaultAuthority,
        rewardMint
      ),
      createAssociatedTokenAccountIdempotentInstruction(
        groupAdmin.wallet.publicKey,
        wrongDestinationAta,
        groupAdmin.wallet.publicKey, // Wrong owner
        rewardMint
      ),
      await kfarmsBankrunProgram.methods
        .refreshUserState()
        .accounts({
          userState,
          farmState,
          scopePrices: null,
        })
        .instruction(),
      await makeKaminoHarvestRewardIx(
        groupAdmin.mrgnBankrunProgram,
        {
          bank: tokenABank,
          feeState,
          destinationTokenAccount: wrongDestinationAta, // Wrong destination
          userState,
          farmState,
          globalConfig,
          rewardMint,
          userRewardAta,
          rewardsVault: rewardVault,
          rewardsTreasuryVault: rewardTreasuryVault,
          farmVaultsAuthority,
          scopePrices: null,
        },
        new BN(0) // reward_index = 0 (first reward)
      )
    );

    const result = await processBankrunTransaction(
      ctx,
      tx,
      [groupAdmin.wallet],
      true
    );
    // ConstraintTokenOwner: 2015 = 0x7df = A token owner constraint was violated.
    assertBankrunTxFailed(result, 2015);
  });

  it("Harvest reward from farm via marginfi", async () => {
    const tokenABank = kaminoAccounts.get(KAMINO_TOKEN_A_BANK);

    const farmState = farmAccounts.get(A_FARM_STATE);
    const globalConfig = farmAccounts.get(GLOBAL_CONFIG);
    const rewardMint = farmAccounts.get(A_REWARD_MINT);
    const userState = farmAccounts.get(A_OBLIGATION_USER_STATE);
    const rewardVault = farmAccounts.get(A_REWARD_VAULT);
    const rewardTreasuryVault = farmAccounts.get(A_REWARD_TREASURY_VAULT);
    const farmVaultsAuthority = farmAccounts.get(A_FARM_VAULTS_AUTHORITY);

    const [liquidityVaultAuthority] = deriveLiquidityVaultAuthority(
      bankrunProgram.programId,
      tokenABank
    );

    const [feeState] = deriveGlobalFeeState(bankrunProgram.programId);

    const userRewardAta = getAssociatedTokenAddressSync(
      rewardMint,
      liquidityVaultAuthority,
      true
    );

    // Destination ATA must be owned by the global fee wallet
    const destinationAta = getAssociatedTokenAddressSync(
      rewardMint,
      globalFeeWallet
    );

    const destinationBalanceBefore = await getTokenBalance(
      bankRunProvider,
      destinationAta
    ).catch(() => 0);

    const tx = new Transaction().add(
      createAssociatedTokenAccountIdempotentInstruction(
        groupAdmin.wallet.publicKey,
        userRewardAta,
        liquidityVaultAuthority,
        rewardMint
      ),
      createAssociatedTokenAccountIdempotentInstruction(
        groupAdmin.wallet.publicKey,
        destinationAta,
        globalFeeWallet,
        rewardMint
      ),
      await kfarmsBankrunProgram.methods
        .refreshUserState()
        .accounts({
          userState,
          farmState,
          scopePrices: null,
        })
        .instruction(),
      await makeKaminoHarvestRewardIx(
        groupAdmin.mrgnBankrunProgram,
        {
          bank: tokenABank,
          feeState,
          destinationTokenAccount: destinationAta,
          userState,
          farmState,
          globalConfig,
          rewardMint,
          userRewardAta,
          rewardsVault: rewardVault,
          rewardsTreasuryVault: rewardTreasuryVault,
          farmVaultsAuthority,
          scopePrices: null,
        },
        new BN(0) // reward_index = 0 (first reward)
      )
    );

    await processBankrunTransaction(ctx, tx, [groupAdmin.wallet], false, true);

    const destinationBalanceAfter = await getTokenBalance(
      bankRunProvider,
      destinationAta
    );
    const harvestedAmount = destinationBalanceAfter - destinationBalanceBefore;

    if (verbose) {
      console.log("Harvested reward amount: " + harvestedAmount);
    }

    assert.isTrue(
      harvestedAmount > 0,
      "Should have harvested some rewards from the farm"
    );
  });

  // TODO approximate whatever our mechanism will be to distribute these rewards to margin users and
  // test that the distribution is approximately fair.
});
