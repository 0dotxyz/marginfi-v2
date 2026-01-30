import { assert } from "chai";
import {
  Keypair,
  Transaction,
  SystemProgram,
  AccountInfo,
} from "@solana/web3.js";
import {
  bankrunContext,
  groupAdmin,
  solendAccounts,
  SOLEND_MARKET,
  bankRunProvider,
} from "./rootHooks";
import { processBankrunTransaction } from "./utils/tools";
import { assertKeysEqual } from "./utils/genericTests";
import {
  makeSolendInitMarketIx,
  SOLEND_PROGRAM_ID,
  PYTH_ORACLE_PROGRAM_ID,
  SWITCHBOARD_ORACLE_PROGRAM_ID,
} from "./utils/solend-sdk";
import {
  SOLEND_MARKET_SEED,
  SOLEND_LENDING_MARKET_SIZE,
  parseSolendLendingMarket,
} from "./utils/solend-utils";

describe("sl01: Init Solend instance", () => {
  const solendMarket = Keypair.fromSeed(SOLEND_MARKET_SEED);

  it("(admin) Initialize lending market", async () => {
    const rentExemptBalance =
      await bankRunProvider.connection.getMinimumBalanceForRentExemption(
        SOLEND_LENDING_MARKET_SIZE
      );

    const createAccountIx = SystemProgram.createAccount({
      fromPubkey: groupAdmin.wallet.publicKey,
      newAccountPubkey: solendMarket.publicKey,
      lamports: rentExemptBalance,
      space: SOLEND_LENDING_MARKET_SIZE,
      programId: SOLEND_PROGRAM_ID,
    });

    const initMarketIx = makeSolendInitMarketIx(
      {
        lendingMarket: solendMarket.publicKey,
        lendingMarketOwner: groupAdmin.wallet.publicKey,
      },
      {
        quoteCurrency: "USD",
      }
    );

    const tx = new Transaction().add(createAccountIx, initMarketIx);

    await processBankrunTransaction(
      bankrunContext,
      tx,
      [groupAdmin.wallet, solendMarket],
      false,
      true
    );

    solendAccounts.set(SOLEND_MARKET, solendMarket.publicKey);

    const marketAccount = await bankrunContext.banksClient.getAccount(
      solendMarket.publicKey
    );

    assert.isNotNull(marketAccount);
    assertKeysEqual(marketAccount.owner, SOLEND_PROGRAM_ID);

    const accountInfo: AccountInfo<Buffer> = {
      data: Buffer.from(marketAccount.data),
      owner: marketAccount.owner,
      lamports: Number(marketAccount.lamports),
      executable: marketAccount.executable,
    };

    const parsedMarket = parseSolendLendingMarket(
      solendMarket.publicKey,
      accountInfo
    );

    assert.equal(parsedMarket.info.version, 1);

    assertKeysEqual(parsedMarket.info.owner, groupAdmin.wallet.publicKey);

    assertKeysEqual(parsedMarket.info.oracleProgramId, PYTH_ORACLE_PROGRAM_ID);

    assertKeysEqual(
      parsedMarket.info.switchboardOracleProgramId,
      SWITCHBOARD_ORACLE_PROGRAM_ID
    );
  });
});
