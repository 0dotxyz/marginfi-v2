import { assert } from "chai";
import {
  ComputeBudgetProgram,
  Keypair,
  PublicKey,
  SystemProgram,
  SYSVAR_RENT_PUBKEY,
  Transaction,
  TransactionInstruction,
  TransactionMessage,
  VersionedTransaction,
} from "@solana/web3.js";
import { TOKEN_PROGRAM_ID } from "@solana/spl-token";
import {
  AssetReserveConfig,
  BorrowRateCurve,
  BorrowRateCurveFields,
  CurvePoint,
  lendingMarketAuthPda,
  LendingMarket,
  MarketWithAddress,
  parseForChangesReserveConfigAndGetIxs,
  PriceFeed,
  Reserve,
  reserveCollateralMintPda,
  reserveCollateralSupplyPda,
  reserveFeeVaultPda,
  reserveLiqSupplyPda,
} from "@kamino-finance/klend-sdk";
import { address } from "@solana/addresses";
import { createNoopSigner } from "@solana/kit";
import Decimal from "decimal.js";
import {
  bankrunContext,
  bankRunProvider,
  groupAdmin,
  kaminoAccounts,
  klendBankrunProgram,
  verbose,
} from "../rootHooks";
import { assertKeysEqual } from "./genericTests";
import { LENDING_MARKET_SIZE, RESERVE_SIZE, toWeb3Ix } from "./kamino-utils";
import {
  createLookupTableForInstructions,
  getBankrunBlockhash,
  processBankrunTransaction,
  processBankrunV0Transaction,
} from "./tools";
import { KLEND_PROGRAM_ID } from "./types";

const toAddress = (pubkey: PublicKey) => address(pubkey.toString());
const toPublicKey = (pubkey: string) => new PublicKey(pubkey);

const encodeQuoteCurrency = (quoteCurrency: string | number[]) => {
  if (Array.isArray(quoteCurrency)) {
    return quoteCurrency;
  }
  return Array.from(quoteCurrency.padEnd(32, "\0")).map((c) => c.charCodeAt(0));
};

export async function createKaminoMarket(
  quote: string | number[] = "USDC"
): Promise<PublicKey> {
  const lendingMarket = Keypair.generate();
  const quoteCurrency = encodeQuoteCurrency(quote);

  const [lendingMarketAuthorityAddress] = await lendingMarketAuthPda(
    toAddress(lendingMarket.publicKey),
    toAddress(klendBankrunProgram.programId)
  );
  const lendingMarketAuthority = toPublicKey(lendingMarketAuthorityAddress);

  const tx = new Transaction().add(
    SystemProgram.createAccount({
      fromPubkey: groupAdmin.wallet.publicKey,
      newAccountPubkey: lendingMarket.publicKey,
      space: LENDING_MARKET_SIZE + 8,
      lamports:
        await bankRunProvider.connection.getMinimumBalanceForRentExemption(
          LENDING_MARKET_SIZE + 8
        ),
      programId: klendBankrunProgram.programId,
    }),
    await klendBankrunProgram.methods
      .initLendingMarket(quoteCurrency)
      .accounts({
        lendingMarketOwner: groupAdmin.wallet.publicKey,
        lendingMarket: lendingMarket.publicKey,
        lendingMarketAuthority,
        systemProgram: SystemProgram.programId,
        rent: SYSVAR_RENT_PUBKEY,
      })
      .instruction()
  );

  await processBankrunTransaction(bankrunContext, tx, [
    groupAdmin.wallet,
    lendingMarket,
  ]);

  return lendingMarket.publicKey;
}

export async function createReserve(
  reserve: Keypair,
  market: PublicKey,
  mint: PublicKey,
  reserveLabel: string,
  decimals: number,
  oracle: PublicKey,
  liquiditySource: PublicKey
) {
  const programAddress = toAddress(klendBankrunProgram.programId);
  const reserveAddress = toAddress(reserve.publicKey);

  const [lendingMarketAuthorityAddress] = await lendingMarketAuthPda(
    toAddress(market),
    programAddress
  );
  const [reserveLiquiditySupplyAddress] = await reserveLiqSupplyPda(
    reserveAddress,
    programAddress
  );
  const [reserveFeeVaultAddress] = await reserveFeeVaultPda(
    reserveAddress,
    programAddress
  );
  const [reserveCollateralMintAddress] = await reserveCollateralMintPda(
    reserveAddress,
    programAddress
  );
  const [reserveCollateralSupplyAddress] = await reserveCollateralSupplyPda(
    reserveAddress,
    programAddress
  );

  const lendingMarketAuthority = toPublicKey(lendingMarketAuthorityAddress);
  const reserveLiquiditySupply = toPublicKey(reserveLiquiditySupplyAddress);
  const feeReceiver = toPublicKey(reserveFeeVaultAddress);
  const reserveCollateralMint = toPublicKey(reserveCollateralMintAddress);
  const reserveCollateralSupply = toPublicKey(reserveCollateralSupplyAddress);

  assertKeysEqual(klendBankrunProgram.programId, KLEND_PROGRAM_ID);

  const tx = new Transaction().add(
    SystemProgram.createAccount({
      fromPubkey: groupAdmin.wallet.publicKey,
      newAccountPubkey: reserve.publicKey,
      space: RESERVE_SIZE + 8,
      lamports:
        await bankRunProvider.connection.getMinimumBalanceForRentExemption(
          RESERVE_SIZE + 8
        ),
      programId: klendBankrunProgram.programId,
    }),
    await klendBankrunProgram.methods
      .initReserve()
      .accountsStrict({
        signer: groupAdmin.wallet.publicKey,
        lendingMarket: market,
        lendingMarketAuthority,
        reserve: reserve.publicKey,
        reserveLiquidityMint: mint,
        reserveLiquiditySupply,
        feeReceiver,
        reserveCollateralMint,
        reserveCollateralSupply,
        initialLiquiditySource: liquiditySource,
        rent: SYSVAR_RENT_PUBKEY,
        liquidityTokenProgram: TOKEN_PROGRAM_ID,
        collateralTokenProgram: TOKEN_PROGRAM_ID,
        systemProgram: SystemProgram.programId,
      })
      .instruction()
  );

  await processBankrunTransaction(bankrunContext, tx, [
    groupAdmin.wallet,
    reserve,
  ]);

  kaminoAccounts.set(reserveLabel, reserve.publicKey);

  if (verbose) {
    console.log("Kamino reserve " + reserveLabel + " " + reserve.publicKey);
  }

  const marketAcc: LendingMarket = LendingMarket.decode(
    (await bankRunProvider.connection.getAccountInfo(market)).data
  );
  const reserveAcc: Reserve = Reserve.decode(
    (await bankRunProvider.connection.getAccountInfo(reserve.publicKey)).data
  );
  assert.equal(reserveAcc.lendingMarket.toString(), market.toString());
  // Reserves start in an unconfigured "Hidden" state.
  assert.equal(reserveAcc.config.status, 2);

  const marketWithAddress: MarketWithAddress = {
    address: toAddress(market),
    state: marketAcc,
  };

  const borrowRateCurve = new BorrowRateCurve({
    points: [
      new CurvePoint({ utilizationRateBps: 0, borrowRateBps: 50000 }),
      new CurvePoint({ utilizationRateBps: 5000, borrowRateBps: 100000 }),
      new CurvePoint({ utilizationRateBps: 8000, borrowRateBps: 500000 }),
      new CurvePoint({ utilizationRateBps: 10000, borrowRateBps: 1000000 }),
      ...Array(7).fill(
        new CurvePoint({ utilizationRateBps: 10000, borrowRateBps: 1000000 })
      ),
    ],
  } as BorrowRateCurveFields);

  const priceFeed: PriceFeed = {
    pythPrice: toAddress(oracle),
  };

  const assetReserveConfig = new AssetReserveConfig({
    mint: toAddress(mint),
    mintTokenProgram: toAddress(TOKEN_PROGRAM_ID),
    tokenName: reserveLabel,
    mintDecimals: decimals,
    priceFeed,
    loanToValuePct: 75,
    liquidationThresholdPct: 85,
    borrowRateCurve,
    depositLimit: new Decimal(1_000_000_000),
    borrowLimit: new Decimal(1_000_000_000),
  }).getReserveConfig();

  const signer = createNoopSigner(toAddress(groupAdmin.wallet.publicKey));
  const instructions: TransactionInstruction[] = [
    ComputeBudgetProgram.setComputeUnitLimit({
      units: 1_400_000,
    }),
  ];

  const ixes = await parseForChangesReserveConfigAndGetIxs(
    marketWithAddress,
    reserveAcc,
    reserveAddress,
    assetReserveConfig,
    programAddress,
    signer
  );

  for (const ix of ixes) {
    instructions.push(toWeb3Ix(ix.ix as any));
  }

  const lutAccount = await createLookupTableForInstructions(
    groupAdmin.wallet,
    instructions
  );

  const messageV0 = new TransactionMessage({
    payerKey: groupAdmin.wallet.publicKey,
    recentBlockhash: await getBankrunBlockhash(bankrunContext),
    instructions,
  }).compileToV0Message([lutAccount]);

  const versionedTx = new VersionedTransaction(messageV0);
  await processBankrunV0Transaction(bankrunContext, versionedTx, [
    groupAdmin.wallet,
  ]);
}
