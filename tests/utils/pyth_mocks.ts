import { BN, Wallet } from "@coral-xyz/anchor";
import { Keypair } from "@solana/web3.js";
import { Oracles } from "./mocks";
import {
  initBlankOracleFeed,
  initOrUpdatePriceUpdateV2,
} from "./pyth-pull-mocks";
import { ORACLE_CONF_INTERVAL } from "./types";

// TODO deprecate standard oracles
/**
 * Set up mock usdc and wsol oracles
 * @param wallet
 * @param wsolPrice
 * @param wsolDecimals
 * @param usdcPrice
 * @param usdcDecimals
 * @param tokenAPrice:
 * @param tokenADecimals:
 * @param tokenBPrice:
 * @param tokenBDecimals:
 * @param verbose
 * @param skips - set to true to skip sending txes, which makes tests run faster if you don't need
 * those oracles.
 * @returns Price oracles for all currencies
 */
export const setupPythOracles = async (
  wallet: Wallet,
  wsolPrice: number,
  wsolDecimals: number,
  usdcPrice: number,
  usdcDecimals: number,
  tokenAPrice: number,
  tokenADecimals: number,
  tokenBPrice: number,
  tokenBDecimals: number,
  lstAlphaPrice: number,
  lstAlphaDecimals: number,
  verbose: boolean,
  skips?: {
    wsol: boolean;
    usdc: boolean;
    a: boolean;
    b: boolean;
  }
) => {
  const now = Math.floor(Date.now() / 1000);
  let wsolPythPullOracle = Keypair.fromSeed(
    Buffer.from("ORACLE_SEED_00000000000000F_WSOL")
  );
  let wsolPythPullOracleFeed = Keypair.fromSeed(
    Buffer.from("ORACLE_SEED_0000000000000ID_WSOL")
  );
  let wsolNativePrice = wsolPrice * 10 ** wsolDecimals;
  let wsolConfidence = wsolNativePrice * ORACLE_CONF_INTERVAL;
  if (skips && skips.wsol) {
    // do nothing
  } else {
    wsolPythPullOracleFeed = await initBlankOracleFeed(wallet);
    wsolPythPullOracle = await initOrUpdatePriceUpdateV2(
      wallet,
      wsolPythPullOracleFeed.publicKey,
      new BN(wsolNativePrice),
      new BN(wsolConfidence),
      now,
      -wsolDecimals
    );
  }

  let usdcPythPullOracle = Keypair.fromSeed(
    Buffer.from("ORACLE_SEED_00000000000000F_USDC")
  );
  let usdcPythPullOracleFeed = Keypair.fromSeed(
    Buffer.from("ORACLE_SEED_0000000000000ID_USDC")
  );
  let usdcNativePrice = usdcPrice * 10 ** usdcDecimals;
  let usdcConfidence = usdcNativePrice * ORACLE_CONF_INTERVAL;
  if (skips && skips.usdc) {
    // do nothing
  } else {
    usdcPythPullOracleFeed = await initBlankOracleFeed(wallet);
    usdcPythPullOracle = await initOrUpdatePriceUpdateV2(
      wallet,
      usdcPythPullOracleFeed.publicKey,
      new BN(usdcNativePrice),
      new BN(usdcConfidence),
      now,
      -usdcDecimals
    );
  }

  let fakeUsdcPythPullOracle = Keypair.fromSeed(
    Buffer.from("ORACLE_SEED_00000000000001F_USDC")
  );
  let fakeUsdcPythPullOracleFeed = Keypair.fromSeed(
    Buffer.from("ORACLE_SEED_0000000000001ID_USDC")
  );
  let fakeUsdcNativePrice = usdcPrice * 10 ** usdcDecimals;
  let fakeUsdcConfidence = fakeUsdcNativePrice * ORACLE_CONF_INTERVAL;
  if (skips && skips.usdc) {
    // do nothing
  } else {
    fakeUsdcPythPullOracleFeed = await initBlankOracleFeed(wallet);
    fakeUsdcPythPullOracle = await initOrUpdatePriceUpdateV2(
      wallet,
      fakeUsdcPythPullOracleFeed.publicKey,
      new BN(fakeUsdcNativePrice),
      new BN(fakeUsdcConfidence),
      now,
      -usdcDecimals
    );
  }

  let tokenAPythPullOracle = Keypair.fromSeed(
    Buffer.from("ORACLE_SEED_00000000000001F_00TA")
  );
  let tokenAPythPullOracleFeed = Keypair.fromSeed(
    Buffer.from("ORACLE_SEED_0000000000001ID_00TA")
  );
  let tokenANativePrice = tokenAPrice * 10 ** tokenADecimals;
  let tokenAConfidence = tokenANativePrice * ORACLE_CONF_INTERVAL;
  if (skips && skips.a) {
    // do nothing
  } else {
    tokenAPythPullOracleFeed = await initBlankOracleFeed(wallet);
    tokenAPythPullOracle = await initOrUpdatePriceUpdateV2(
      wallet,
      tokenAPythPullOracleFeed.publicKey,
      new BN(tokenANativePrice),
      new BN(tokenAConfidence),
      now,
      -tokenADecimals
    );
  }

  let tokenBPythPullOracle = Keypair.fromSeed(
    Buffer.from("ORACLE_SEED_00000000000001F_00TB")
  );
  let tokenBPythPullOracleFeed = Keypair.fromSeed(
    Buffer.from("ORACLE_SEED_0000000000001ID_00TB")
  );
  let tokenBNativePrice = tokenBPrice * 10 ** tokenBDecimals;
  let tokenBConfidence = tokenBNativePrice * ORACLE_CONF_INTERVAL;
  if (skips && skips.b) {
    // do nothing
  } else {
    tokenBPythPullOracleFeed = await initBlankOracleFeed(wallet);
    tokenBPythPullOracle = await initOrUpdatePriceUpdateV2(
      wallet,
      tokenBPythPullOracleFeed.publicKey,
      new BN(tokenBNativePrice),
      new BN(tokenBConfidence),
      now,
      -tokenBDecimals
    );
  }

  let lstPythPullOracle = Keypair.fromSeed(
    Buffer.from("ORACLE_SEED_00000000000001F_0LST")
  );
  let lstPythPullOracleFeed = Keypair.fromSeed(
    Buffer.from("ORACLE_SEED_0000000000001ID_0LST")
  );
  let priceAlpha = lstAlphaPrice * 10 ** lstAlphaDecimals;
  let confAlpha = priceAlpha * ORACLE_CONF_INTERVAL;
  if (skips && skips.wsolPyth) {
    // do nothing
  } else {
    lstPythPullOracleFeed = await initBlankOracleFeed(
      wallet,
      lstPythPullOracleFeed
    );
    lstPythPullOracle = await initOrUpdatePriceUpdateV2(
      wallet,
      lstPythPullOracleFeed.publicKey,
      new BN(priceAlpha),
      new BN(confAlpha),
      now,
      -lstAlphaDecimals,
      undefined,
      lstPythPullOracle
    );
  }

  if (verbose) {
    console.log("Mock Pyth Pull price oracles:");
    console.log("wsol:    \t" + wsolPythPullOracle.publicKey);
    console.log("usdc:    \t" + usdcPythPullOracle.publicKey);
    console.log("token a: \t" + tokenAPythPullOracle.publicKey);
    console.log("token b: \t" + tokenBPythPullOracle.publicKey);
    console.log("lst:     \t" + lstPythPullOracle.publicKey);
    console.log(
      "Price of 1 wsol.......$" +
        wsolPrice +
        "\t  one token in native decimals: " +
        (1 * 10 ** wsolDecimals).toLocaleString()
    );
    console.log(
      "Price of 1 usdc.......$" +
        usdcPrice +
        "\t  one token in native decimals: " +
        (1 * 10 ** usdcDecimals).toLocaleString()
    );
    console.log(
      "Price of 1 token A....$" +
        tokenAPrice +
        "\t  one token in native decimals: " +
        (1 * 10 ** tokenADecimals).toLocaleString()
    );
    console.log(
      "Price of 1 token B....$" +
        tokenBPrice +
        "\t  one token in native decimals: " +
        (1 * 10 ** tokenBDecimals).toLocaleString()
    );
    console.log(
      "Price of 1 LST alpha..$" +
        lstAlphaPrice +
        "\t  one token in native decimals: " +
        (1 * 10 ** lstAlphaDecimals).toLocaleString()
    );
    console.log("");
  }
  let oracles: Oracles = {
    wsolOracle: wsolPythPullOracle,
    wsolOracleFeed: wsolPythPullOracleFeed,
    wsolDecimals: wsolDecimals,
    usdcOracle: usdcPythPullOracle,
    usdcOracleFeed: usdcPythPullOracleFeed,
    usdcDecimals: usdcDecimals,
    tokenAOracle: tokenAPythPullOracle,
    tokenAOracleFeed: tokenAPythPullOracleFeed,
    tokenADecimals: tokenADecimals,
    tokenBOracle: tokenBPythPullOracle,
    tokenBOracleFeed: tokenBPythPullOracleFeed,
    tokenBDecimals: tokenBDecimals,
    wsolPrice: wsolPrice,
    usdcPrice: usdcPrice,
    tokenAPrice: tokenAPrice,
    tokenBPrice: tokenBPrice,
    lstAlphaPrice: lstAlphaPrice,
    lstAlphaDecimals: lstAlphaDecimals,
    fakeUsdc: fakeUsdcPythPullOracle.publicKey,
    fakeUsdcFeed: fakeUsdcPythPullOracleFeed.publicKey,
    pythPullLst: lstPythPullOracle,
    pythPullLstOracleFeed: lstPythPullOracleFeed,
    // Legacy/mock suite placeholder values for Switchboard fields.
    wsolOracleSwb: wsolPythPullOracle,
    wsolPriceSwb: wsolPrice,
    wsolDecimalsSwb: wsolDecimals,
    tokenAOracleSwb: tokenAPythPullOracle,
    tokenAPriceSwb: tokenAPrice,
    tokenADecimalsSwb: tokenADecimals,
    lstAlphaOracleSwb: lstPythPullOracle,
    lstAlphaPriceSwb: lstAlphaPrice,
    lstAlphaDecimalsSwb: lstAlphaDecimals,
  };
  return oracles;
};

async function setupPull(
  wallet: Wallet,
  price: number,
  decimals: number,
  oracleConfDefault: number
): Promise<[Keypair, Keypair]> {
  // compute BN-scaled values
  const priceBn = new BN(price * 10 ** decimals);
  const confBn = new BN(price * oracleConfDefault * 10 ** decimals);

  // overwrite feed/oracle
  const newFeed = await initBlankOracleFeed(wallet);
  const newOracle = await initOrUpdatePriceUpdateV2(
    wallet,
    newFeed.publicKey,
    priceBn,
    confBn,
    priceBn,
    confBn,
    new BN(0),
    -decimals
  );

  return [newOracle, newFeed];
}
