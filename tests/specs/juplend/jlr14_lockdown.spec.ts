import {
  ComputeBudgetProgram,
  Keypair,
  PublicKey,
  Transaction,
  TransactionInstruction,
} from "@solana/web3.js";
import {
  createAssociatedTokenAccountIdempotentInstruction,
  getAssociatedTokenAddressSync,
} from "@solana/spl-token";
import { assert } from "chai";
import { BN } from "@coral-xyz/anchor";
import BigNumber from "bignumber.js";
import {
  bigNumberToWrappedI80F48,
  wrappedI80F48toBigNumber,
} from "@mrgnlabs/mrgn-common";

import {
  bankrunContext,
  bankrunProgram,
  banksClient,
  ecosystem,
  globalProgramAdmin,
  groupAdmin,
  juplendAccounts,
  oracles,
  users,
} from "../../rootHooks";
import { assertBankrunTxFailed } from "../../utils/genericTests";
import {
  refreshPullOraclesBankrun,
  refreshSwitchboardPullOracleBankrun,
} from "../../utils/bankrun-oracles";
import {
  accountInit,
  borrowIx,
  composeRemainingAccounts,
  depositIx,
  healthPulse,
  liquidateIx,
  pulseBankPrice,
  repayIx,
} from "../../utils/user-instructions";
import {
  buildHealthRemainingAccounts,
  mintToTokenAccount,
  processBankrunTransaction,
} from "../../utils/tools";
import { createMintBankrun } from "../../utils/mocks";
import {
  configureBank,
  configureBankOracle,
} from "../../utils/group-instructions";
import {
  ASSET_TAG_JUPLEND,
  HEALTH_CACHE_ORACLE_OK,
  ORACLE_SETUP_JUPLEND_LST,
  ORACLE_SETUP_JUPLEND_MSOL,
  blankBankConfigOptRaw,
} from "../../utils/types";
import {
  deriveBankWithSeed,
  deriveLiquidityVaultAuthority,
} from "../../utils/pdas";
import {
  deriveJuplendPoolKeys,
  findJuplendLiquidityPda,
  JUPLEND_LIQUIDITY_PROGRAM_ID,
} from "../../utils/juplend/juplend-pdas";
import {
  JUPLEND_STATE_KEYS,
  jlr01BankStateKey,
} from "../../utils/juplend/test-state";
import { getJuplendPrograms } from "../../utils/juplend/programs";
import { refreshJupSimple } from "../../utils/juplend/shorthand-instructions";
import { makeJuplendDepositIx } from "../../utils/juplend/user-instructions";
import {
  addJuplendBankIx,
  makeJuplendInitPositionIx,
} from "../../utils/juplend/group-instructions";
import {
  configureJuplendProtocolPermissions,
  initJuplendPool,
} from "../../utils/juplend/jlr-pool-setup";
import {
  DEFAULT_BORROW_CONFIG_MIN,
  defaultJuplendBankConfig,
} from "../../utils/juplend/types";

// ---------------------------------------------------------------------------
// Constants & types
// ---------------------------------------------------------------------------

const ERR_WRONG_NUMBER_OF_ORACLE_ACCOUNTS = 6051;
const ERR_LIQUIDITY_VALIDATION = 6514;
const ERR_RISK_ENGINE_INIT_REJECTED = 6009;

const PRICE_EPSILON = new BigNumber("1e-12");

// JupLend Liquidity layout: disc (8) + authority (32) + revenue collector (32) + status (1) + bump (1).
const LIQUIDITY_STATUS_OFFSET = 8 + 32 + 32;
const LIQUIDITY_LEN = LIQUIDITY_STATUS_OFFSET + 2;
const LIQUIDITY_DISCRIMINATOR = [54, 252, 249, 226, 137, 172, 121, 58];
const Status = { ACTIVE: 0, LOCKED: 1, UNKNOWN: 2 } as const;

const LIQUIDITY_PK = findJuplendLiquidityPda()[0];
const FIXTURE_DECIMALS = 9;

const SWITCHBOARD_SETUPS = new Set([
  "juplendSwitchboardPull",
  "switchboardPull",
]);

// Indices into bank.config.oracleKeys each adapter prices from; the Liquidity PDA is always appended.
const PRICING_KEY_INDICES: Record<string, number[]> = {
  fixedJuplend: [1],
  juplendPythPull: [0, 1],
  juplendSwitchboardPull: [0, 1],
  scopeJuplend: [0, 1],
  juplendMsol: [0, 1, 2],
  juplendLst: [0, 1, 2],
};

type TestUser = (typeof users)[number];
type BankrunAccount = NonNullable<
  Awaited<ReturnType<typeof banksClient.getAccount>>
>;
type TxResult = Parameters<typeof assertBankrunTxFailed>[0];
type Bank = Awaited<ReturnType<typeof fetchBank>>;
type PoolKeys = ReturnType<typeof deriveJuplendPoolKeys>;
type InitializedPool = Awaited<ReturnType<typeof initJuplendPool>>;
type PricedBank = { bankPk: PublicKey; pricing: PublicKey[] };
type Market = {
  group: PublicKey;
  collateral: PricedBank;
  liabilityBankPk: PublicKey;
  pool: PoolKeys;
};

interface LockdownFixture {
  stateKey: string;
  seed: number;
  symbol: string;
  oracleSetup:
    | typeof ORACLE_SETUP_JUPLEND_MSOL
    | typeof ORACLE_SETUP_JUPLEND_LST;
  pricingSource: PublicKey;
  mintOffset: number;
}

interface Scenario {
  name: string;
  expectedSetup: string;
  bankKey: string;
  hint: string;
}

// mSOL/LST banks are created here with pricing accounts cloned from the repository's bankrun fixtures.
const LOCKDOWN_FIXTURES: Record<"msol" | "lst", LockdownFixture> = {
  msol: {
    stateKey: JUPLEND_STATE_KEYS.jlr14BankMsolJuplend,
    seed: 91401,
    symbol: "J14MSOL",
    oracleSetup: ORACLE_SETUP_JUPLEND_MSOL,
    pricingSource: new PublicKey(
      "8szGkuLTAux9XMgZ2vtY39jVSowEcpBfFfD8hXSEqdGC"
    ),
    mintOffset: 8,
  },
  lst: {
    stateKey: JUPLEND_STATE_KEYS.jlr14BankLstJuplend,
    seed: 91402,
    symbol: "J14LST",
    oracleSetup: ORACLE_SETUP_JUPLEND_LST,
    pricingSource: new PublicKey(
      "9mhGNSPArRMHpLDMSmxAvuoizBqtBGqYdT8WGuqgxNdn"
    ),
    mintOffset: 162,
  },
};

const SCENARIOS: Scenario[] = [
  {
    name: "JuplendPythPull",
    expectedSetup: "juplendPythPull",
    bankKey: jlr01BankStateKey("USDC"),
    hint: "jlr01 must register the USDC JupLend bank.",
  },
  {
    name: "JuplendSwitchboardPull",
    expectedSetup: "juplendSwitchboardPull",
    bankKey: JUPLEND_STATE_KEYS.jlr08BankUsdcSwitchboard,
    hint: "jlr08 must register its Switchboard bank.",
  },
  {
    name: "FixedJuplend",
    expectedSetup: "fixedJuplend",
    bankKey: JUPLEND_STATE_KEYS.jlr07BankUsdcFixed,
    hint: "jlr07 must register its fixed-price bank.",
  },
  {
    name: "ScopeJuplend",
    expectedSetup: "scopeJuplend",
    bankKey: JUPLEND_STATE_KEYS.jlr13BankUsdcScope,
    hint: "jlr13 must register its Scope bank.",
  },
  {
    name: "JuplendMSOL",
    expectedSetup: "juplendMsol",
    bankKey: LOCKDOWN_FIXTURES.msol.stateKey,
    hint: "The jlr14 mSOL fixture must be initialized.",
  },
  {
    name: "JuplendLST",
    expectedSetup: "juplendLst",
    bankKey: LOCKDOWN_FIXTURES.lst.stateKey,
    hint: "The jlr14 LST fixture must be initialized.",
  },
];

const PRIMARY = SCENARIOS[0];

// ---------------------------------------------------------------------------
// Small helpers
// ---------------------------------------------------------------------------

const toBaseUnits = (amount: number, decimals: number): BN =>
  new BN(amount).mul(new BN(10).pow(new BN(decimals)));
const usdc = (amount: number) => toBaseUnits(amount, ecosystem.usdcDecimals);
const tokenB = (amount: number) =>
  toBaseUnits(amount, ecosystem.tokenBDecimals);

const fetchBank = (bankPk: PublicKey) =>
  bankrunProgram.account.bank.fetch(bankPk);
const oracleSetupOf = (bank: Bank): string =>
  Object.keys(bank.config.oracleSetup)[0];
const cachedPrice = (bank: Bank) =>
  wrappedI80F48toBigNumber(bank.cache.lastOraclePrice);
const isPriceUnchanged = (price: BigNumber, baseline: BigNumber) =>
  price.minus(baseline).abs().lte(PRICE_EPSILON);
const netValue = (assets: any, liabilities: any) =>
  wrappedI80F48toBigNumber(assets).minus(wrappedI80F48toBigNumber(liabilities));

const adminProgram = () => groupAdmin.mrgnBankrunProgram!;

const getRegisteredBank = (key: string, errorMessage: string): PublicKey => {
  const bankPk = juplendAccounts.get(key);
  if (!bankPk) throw new Error(errorMessage);
  return bankPk;
};

const resolveBank = (s: Scenario): PublicKey =>
  getRegisteredBank(
    s.bankKey,
    `${s.name}: no bank registered under "${s.bankKey}". ${s.hint}`
  );

const repayOrThrow = async (label: string, repay: () => Promise<void>) => {
  try {
    await repay();
  } catch (e) {
    throw new Error(`jlr14 ${label} cleanup (repay) failed: ${e}`);
  }
};

// ---------------------------------------------------------------------------
// Liquidity account: snapshot, mutate, restore
// ---------------------------------------------------------------------------

class LiquidityAccount {
  private pristine?: BankrunAccount;

  async capture() {
    const account = await banksClient.getAccount(LIQUIDITY_PK);
    if (!account) {
      throw new Error(
        "JupLend Liquidity account missing from bankrun state; run jlr01 first / fix test ordering"
      );
    }
    assert.isTrue(
      account.owner.equals(JUPLEND_LIQUIDITY_PROGRAM_ID),
      "Liquidity account is not owned by the JupLend liquidity program"
    );
    assert.deepEqual(
      Array.from(account.data.slice(0, 8)),
      LIQUIDITY_DISCRIMINATOR,
      "unexpected Liquidity discriminator"
    );
    assert.equal(
      account.data.length,
      LIQUIDITY_LEN,
      "Liquidity account length changed"
    );
    assert.equal(
      account.data[LIQUIDITY_STATUS_OFFSET],
      Status.ACTIVE,
      "baseline Liquidity status must be active"
    );
    this.pristine = account;
  }

  async setStatus(status: number) {
    this.overwrite({ data: this.dataWithStatus(status) });
    const after = await banksClient.getAccount(LIQUIDITY_PK);
    assert.isNotNull(after, "Liquidity account vanished after setAccount");
    assert.equal(
      after!.data[LIQUIDITY_STATUS_OFFSET],
      status,
      "Liquidity status byte was not updated"
    );
  }

  corruptDiscriminator() {
    const data = this.dataWithStatus(Status.ACTIVE);
    data[0] ^= 0xff;
    this.overwrite({ data });
  }

  useWrongOwner() {
    this.overwrite({ owner: Keypair.generate().publicKey });
  }

  truncate() {
    this.overwrite({
      data: this.dataWithStatus(Status.ACTIVE).subarray(0, LIQUIDITY_LEN - 1),
    });
  }

  restore() {
    if (this.pristine) bankrunContext.setAccount(LIQUIDITY_PK, this.pristine);
  }

  private get snapshot(): BankrunAccount {
    if (!this.pristine)
      throw new Error("Liquidity snapshot has not been captured");
    return this.pristine;
  }

  private dataWithStatus(status: number): Buffer {
    const data = Buffer.from(this.snapshot.data);
    data[LIQUIDITY_STATUS_OFFSET] = status;
    return data;
  }

  private overwrite(patch: { data?: Uint8Array; owner?: PublicKey }) {
    bankrunContext.setAccount(LIQUIDITY_PK, {
      ...this.snapshot,
      ...patch,
    } as BankrunAccount);
  }
}

// ---------------------------------------------------------------------------
// Transactions
// ---------------------------------------------------------------------------

class TxRunner {
  private nonce = 0;

  // A distinct compute-unit limit keeps bankrun from deduplicating identical transactions.
  build(...ixs: TransactionInstruction[]): Transaction {
    return new Transaction().add(
      ComputeBudgetProgram.setComputeUnitLimit({
        units: 1_000_000 - this.nonce++,
      }),
      ...ixs
    );
  }

  send(tx: Transaction, signers: Keypair[] = [users[0].wallet]) {
    return processBankrunTransaction(bankrunContext, tx, signers, false, false);
  }

  trySend(tx: Transaction, signers: Keypair[] = [users[0].wallet]) {
    return processBankrunTransaction(
      bankrunContext,
      tx,
      signers,
      true,
      false
    ) as Promise<TxResult>;
  }

  run(ixs: TransactionInstruction[], signers?: Keypair[]) {
    return this.send(this.build(...ixs), signers);
  }
}

const liquidity = new LiquidityAccount();
const txs = new TxRunner();

const sendAsAdmin = (...ixs: TransactionInstruction[]) =>
  processBankrunTransaction(bankrunContext, new Transaction().add(...ixs), [
    groupAdmin.wallet,
  ]);

// ---------------------------------------------------------------------------
// Bank preparation & pricing
// ---------------------------------------------------------------------------

const refreshJuplendIx = (pool: PoolKeys) =>
  refreshJupSimple(getJuplendPrograms().lending, { pool });

const prepareBankState = async (bankPk: PublicKey) => {
  const bank = await fetchBank(bankPk);
  const refreshIxs =
    bank.config.assetTag === ASSET_TAG_JUPLEND
      ? [await refreshJuplendIx(deriveJuplendPoolKeys({ mint: bank.mint }))]
      : [];

  await refreshPullOraclesBankrun(oracles, bankrunContext, banksClient);
  if (SWITCHBOARD_SETUPS.has(oracleSetupOf(bank))) {
    await refreshSwitchboardPullOracleBankrun(
      bankrunContext,
      banksClient,
      bank.config.oracleKeys[0]
    );
  }

  if (refreshIxs.length > 0) await txs.run(refreshIxs);
};

const pricingAccounts = async (
  bankPk: PublicKey,
  expectedSetup: string
): Promise<PublicKey[]> => {
  const bank = await fetchBank(bankPk);
  const setup = oracleSetupOf(bank);
  assert.equal(
    setup,
    expectedSetup,
    `bank ${bankPk.toBase58()} has an unexpected oracle setup`
  );

  const indices = PRICING_KEY_INDICES[setup];
  if (!indices) throw new Error(`Unsupported JupLend oracle setup: ${setup}`);

  const accounts = [
    ...indices.map((i) => bank.config.oracleKeys[i]),
    LIQUIDITY_PK,
  ];
  accounts.forEach((pk, i) =>
    assert.isFalse(
      pk.equals(PublicKey.default),
      `${setup}: pricing account #${i} is the default pubkey`
    )
  );
  return accounts;
};

const loadPricedBank = async (s: Scenario): Promise<PricedBank> => {
  const bankPk = resolveBank(s);
  return { bankPk, pricing: await pricingAccounts(bankPk, s.expectedSetup) };
};

const loadPreparedBank = async (s: Scenario): Promise<PricedBank> => {
  const priced = await loadPricedBank(s);
  await prepareBankState(priced.bankPk);
  return priced;
};

const pulseTx = async ({ bankPk, pricing }: PricedBank) =>
  txs.build(
    await pulseBankPrice(users[0].mrgnBankrunProgram!, {
      bank: bankPk,
      remaining: pricing,
    })
  );

const pulseAndFetch = async (priced: PricedBank) => {
  await txs.send(await pulseTx(priced));
  return fetchBank(priced.bankPk);
};

const pulseWithStatus = async (priced: PricedBank, status: number) => {
  await liquidity.setStatus(status);
  return pulseAndFetch(priced);
};

// ---------------------------------------------------------------------------
// Fixture creation for the otherwise-uncovered JuplendMSOL and JuplendLST adapters
// ---------------------------------------------------------------------------

const clonePricingAccountWithMint = async (
  source: PublicKey,
  mintOffset: number,
  mint: PublicKey
): Promise<PublicKey> => {
  const sourceAccount = await banksClient.getAccount(source);
  if (!sourceAccount) {
    throw new Error(`Missing pricing fixture ${source.toBase58()}`);
  }

  const data = Buffer.from(sourceAccount.data);
  mint.toBuffer().copy(data, mintOffset);
  const clonedKey = Keypair.generate().publicKey;
  bankrunContext.setAccount(clonedKey, { ...sourceAccount, data });
  return clonedKey;
};

const createFundedMint = async () => {
  const mintKeypair = Keypair.generate();
  const mint = mintKeypair.publicKey;
  await createMintBankrun(
    bankrunContext,
    globalProgramAdmin.wallet,
    FIXTURE_DECIMALS,
    mintKeypair
  );

  const adminTokenAccount = getAssociatedTokenAddressSync(
    mint,
    groupAdmin.wallet.publicKey
  );
  await sendAsAdmin(
    createAssociatedTokenAccountIdempotentInstruction(
      groupAdmin.wallet.publicKey,
      adminTokenAccount,
      groupAdmin.wallet.publicKey,
      mint
    )
  );
  await mintToTokenAccount(
    mint,
    adminTokenAccount,
    toBaseUnits(10, FIXTURE_DECIMALS)
  );
  return { mint, adminTokenAccount };
};

const createPool = async (mint: PublicKey, symbol: string) => {
  const pool = await initJuplendPool({
    admin: groupAdmin.wallet,
    mint,
    symbol,
    decimals: FIXTURE_DECIMALS,
  });
  await configureJuplendProtocolPermissions({
    admin: groupAdmin.wallet,
    mint,
    lending: pool.lending,
    rateModel: pool.rateModel,
    tokenReserve: pool.tokenReserve,
    supplyPositionOnLiquidity: pool.supplyPositionOnLiquidity,
    borrowPositionOnLiquidity: pool.borrowPositionOnLiquidity,
    tokenProgram: pool.tokenProgram,
    borrowConfig: DEFAULT_BORROW_CONFIG_MIN,
  });
  return pool;
};

const registerBank = async (
  group: PublicKey,
  mint: PublicKey,
  seed: number,
  pool: InitializedPool
): Promise<PublicKey> => {
  const bankSeed = new BN(seed);
  const [bank] = deriveBankWithSeed(
    bankrunProgram.programId,
    group,
    mint,
    bankSeed
  );
  await sendAsAdmin(
    await addJuplendBankIx(adminProgram(), {
      group,
      feePayer: groupAdmin.wallet.publicKey,
      bankMint: mint,
      bankSeed,
      oracle: oracles.wsolOracle.publicKey,
      jupLendingState: pool.lending,
      fTokenMint: pool.fTokenMint,
      config: defaultJuplendBankConfig(
        oracles.wsolOracle.publicKey,
        FIXTURE_DECIMALS
      ),
      tokenProgram: pool.tokenProgram,
    })
  );
  return bank;
};

const seedPosition = async (
  bank: PublicKey,
  mint: PublicKey,
  pool: InitializedPool,
  adminTokenAccount: PublicKey
) => {
  const { integrationAcc3 } = await fetchBank(bank);
  const vaultAuthority = deriveLiquidityVaultAuthority(
    bankrunProgram.programId,
    bank
  )[0];
  await sendAsAdmin(
    createAssociatedTokenAccountIdempotentInstruction(
      groupAdmin.wallet.publicKey,
      integrationAcc3,
      vaultAuthority,
      mint,
      pool.tokenProgram
    ),
    await makeJuplendInitPositionIx(adminProgram(), {
      feePayer: groupAdmin.wallet.publicKey,
      signerTokenAccount: adminTokenAccount,
      bank,
      pool,
      seedDepositAmount: toBaseUnits(1, FIXTURE_DECIMALS),
      tokenProgram: pool.tokenProgram,
    })
  );
};

const attachPricing = async (
  bank: PublicKey,
  mint: PublicKey,
  pool: InitializedPool,
  fixture: LockdownFixture
) => {
  const pricingState = await clonePricingAccountWithMint(
    fixture.pricingSource,
    fixture.mintOffset,
    mint
  );
  await sendAsAdmin(
    await configureBankOracle(adminProgram(), {
      bank,
      type: fixture.oracleSetup,
      oracle: oracles.wsolOracle.publicKey,
      remaining: [pool.lending, pricingState, LIQUIDITY_PK],
    })
  );
};

const createLockdownBank = async (
  fixture: LockdownFixture
): Promise<PublicKey> => {
  const registered = juplendAccounts.get(fixture.stateKey);
  if (registered) return registered;

  const { group } = await fetchBank(resolveBank(PRIMARY));
  const { mint, adminTokenAccount } = await createFundedMint();
  const pool = await createPool(mint, fixture.symbol);
  const bank = await registerBank(group, mint, fixture.seed, pool);
  await seedPosition(bank, mint, pool, adminTokenAccount);
  await attachPricing(bank, mint, pool, fixture);

  juplendAccounts.set(fixture.stateKey, bank);
  return bank;
};

// ---------------------------------------------------------------------------
// Marginfi actor (USDC collateral on JupLend, Token B liability)
// ---------------------------------------------------------------------------

class Actor {
  private readonly keypair = Keypair.generate();
  private readonly user: TestUser;
  private readonly group: PublicKey;

  constructor(user: TestUser, group: PublicKey) {
    this.user = user;
    this.group = group;
  }

  get account(): PublicKey {
    return this.keypair.publicKey;
  }

  private get wallet(): Keypair {
    return this.user.wallet;
  }

  private get program() {
    return this.user.mrgnBankrunProgram!;
  }

  async init() {
    await txs.run(
      [
        await accountInit(this.program, {
          marginfiGroup: this.group,
          marginfiAccount: this.account,
          authority: this.wallet.publicKey,
          feePayer: this.wallet.publicKey,
        }),
      ],
      [this.wallet, this.keypair]
    );
  }

  async depositToJuplend(market: Market, amount: BN) {
    await mintToTokenAccount(
      ecosystem.usdcMint.publicKey,
      this.user.usdcAccount,
      amount
    );
    await txs.run(
      [
        await refreshJuplendIx(market.pool),
        await makeJuplendDepositIx(this.program, {
          marginfiAccount: this.account,
          signerTokenAccount: this.user.usdcAccount,
          bank: market.collateral.bankPk,
          pool: market.pool,
          amount,
        }),
      ],
      [this.wallet]
    );
  }

  async depositUsdc(bank: PublicKey, amount: BN) {
    await mintToTokenAccount(
      ecosystem.usdcMint.publicKey,
      this.user.usdcAccount,
      amount
    );
    await txs.run(
      [
        await depositIx(this.program, {
          marginfiAccount: this.account,
          bank,
          tokenAccount: this.user.usdcAccount,
          amount,
        }),
      ],
      [this.wallet]
    );
  }

  async borrow(bank: PublicKey, amount: BN) {
    await txs.send(await this.borrowTx(bank, amount), [this.wallet]);
  }

  async tryBorrow(bank: PublicKey, amount: BN) {
    return txs.trySend(await this.borrowTx(bank, amount), [this.wallet]);
  }

  async repayAll(bank: PublicKey, amount: BN, topUp: BN = amount) {
    await mintToTokenAccount(
      ecosystem.tokenBMint.publicKey,
      this.user.tokenBAccount,
      topUp
    );
    const remaining = await buildHealthRemainingAccounts(this.account);
    await txs.run(
      [
        await repayIx(this.program, {
          marginfiAccount: this.account,
          bank,
          tokenAccount: this.user.tokenBAccount,
          amount,
          remaining,
          repayAll: true,
        }),
      ],
      [this.wallet]
    );
  }

  async pulseHealth() {
    const remaining = await buildHealthRemainingAccounts(this.account);
    await txs.run(
      [
        await healthPulse(this.program, {
          marginfiAccount: this.account,
          remaining,
        }),
      ],
      [this.wallet]
    );
    const { healthCache: cache } =
      await bankrunProgram.account.marginfiAccount.fetch(this.account);
    return {
      initHealth: netValue(cache.assetValue, cache.liabilityValue),
      maintHealth: netValue(cache.assetValueMaint, cache.liabilityValueMaint),
      flags: cache.flags,
    };
  }

  async liquidate(args: {
    assetBank: PublicKey;
    liabilityBank: PublicKey;
    liquidatee: Actor;
    amount: BN;
    accounts: LiquidationAccounts;
  }) {
    await txs.run(
      [
        await liquidateIx(this.program, {
          assetBankKey: args.assetBank,
          liabilityBankKey: args.liabilityBank,
          liquidatorMarginfiAccount: this.account,
          liquidateeMarginfiAccount: args.liquidatee.account,
          amount: args.amount,
          ...args.accounts,
        }),
      ],
      [this.wallet]
    );
  }

  private async borrowTx(bank: PublicKey, amount: BN) {
    const remaining = await buildHealthRemainingAccounts(this.account, {
      includedBankPks: [bank],
    });
    return txs.build(
      await borrowIx(this.program, {
        marginfiAccount: this.account,
        bank,
        tokenAccount: this.user.tokenBAccount,
        amount,
        remaining,
      })
    );
  }
}

// ---------------------------------------------------------------------------
// Scenario scaffolding
// ---------------------------------------------------------------------------

const loadMarket = async (): Promise<Market> => {
  const collateral = await loadPricedBank(PRIMARY);
  const liabilityBankPk = getRegisteredBank(
    JUPLEND_STATE_KEYS.jlr01RegularBankTokenB,
    "JLR01 regular Token B bank is missing"
  );
  const collateralBank = await fetchBank(collateral.bankPk);
  const liabilityBank = await fetchBank(liabilityBankPk);
  assert.isTrue(collateralBank.mint.equals(ecosystem.usdcMint.publicKey));
  assert.isTrue(liabilityBank.mint.equals(ecosystem.tokenBMint.publicKey));

  return {
    group: collateralBank.group,
    collateral,
    liabilityBankPk,
    pool: deriveJuplendPoolKeys({ mint: collateralBank.mint }),
  };
};

const fundCollateral = async (actor: Actor, market: Market, amount: BN) => {
  await liquidity.setStatus(Status.ACTIVE);
  await prepareBankState(market.collateral.bankPk);
  await actor.init();
  await actor.depositToJuplend(market, amount);
};

const setLiabilityWeights = async (
  group: PublicKey,
  bank: PublicKey,
  init: Bank["config"]["liabilityWeightInit"],
  maint: Bank["config"]["liabilityWeightMaint"]
) => {
  const bankConfigOpt = blankBankConfigOptRaw();
  bankConfigOpt.liabilityWeightInit = init;
  bankConfigOpt.liabilityWeightMaint = maint;
  await txs.run(
    [await configureBank(adminProgram(), { group, bank, bankConfigOpt })],
    [groupAdmin.wallet]
  );
};

type LiquidationAccounts = {
  remaining: PublicKey[];
  liquidateeAccounts: number;
  liquidatorAccounts: number;
};

const buildLiquidationAccounts = async (
  market: Market,
  liquidatorUsdcBankPk: PublicKey
): Promise<LiquidationAccounts> => {
  const liabilityBank = await fetchBank(market.liabilityBankPk);
  const liquidatorUsdcBank = await fetchBank(liquidatorUsdcBankPk);

  const jupSet = [market.collateral.bankPk, ...market.collateral.pricing];
  const liabilityOracle = liabilityBank.config.oracleKeys[0];
  const liabilitySet = [market.liabilityBankPk, liabilityOracle];
  const liquidatorUsdcSet = [
    liquidatorUsdcBankPk,
    liquidatorUsdcBank.config.oracleKeys[0],
  ];

  const liquidatee = composeRemainingAccounts([jupSet, liabilitySet]);
  const liquidator = composeRemainingAccounts([
    liquidatorUsdcSet,
    jupSet,
    liabilitySet,
  ]);

  return {
    remaining: [
      ...market.collateral.pricing,
      liabilityOracle,
      ...liquidator,
      ...liquidatee,
    ],
    liquidateeAccounts: liquidatee.length,
    liquidatorAccounts: liquidator.length,
  };
};

const LOCK_TRANSITIONS: Array<{
  label: string;
  apply: () => Promise<void> | void;
  expectedDisabled: number;
}> = [
  {
    label: "locked",
    apply: () => liquidity.setStatus(Status.LOCKED),
    expectedDisabled: 1,
  },
  {
    label: "unlocked",
    apply: () => liquidity.setStatus(Status.ACTIVE),
    expectedDisabled: 0,
  },
  {
    label: "unknown status",
    apply: () => liquidity.setStatus(Status.UNKNOWN),
    expectedDisabled: 1,
  },
  {
    label: "re-unlocked",
    apply: () => liquidity.setStatus(Status.ACTIVE),
    expectedDisabled: 0,
  },
  {
    label: "unreadable Liquidity",
    apply: () => liquidity.corruptDiscriminator(),
    expectedDisabled: 1,
  },
  {
    label: "recovered",
    apply: () => liquidity.setStatus(Status.ACTIVE),
    expectedDisabled: 0,
  },
];

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

describe("jlr14: JupLend protocol lockdown (bankrun)", () => {
  before(async () => {
    await liquidity.capture();
    for (const fixture of Object.values(LOCKDOWN_FIXTURES)) {
      await createLockdownBank(fixture);
    }
    // Pool setup can write to the global Liquidity account, so snapshot again after it.
    await liquidity.capture();
  });

  afterEach(() => liquidity.restore());
  after(() => liquidity.restore());

  describe("liquidity account validation", () => {
    const disablingFaults: Array<[string, () => Promise<void> | void]> = [
      [
        "locked status disables borrow power",
        () => liquidity.setStatus(Status.LOCKED),
      ],
      [
        "unreadable bad discriminator disables borrow power without breaking pricing",
        () => liquidity.corruptDiscriminator(),
      ],
      [
        "unreadable wrong owner disables borrow power without breaking pricing",
        () => liquidity.useWrongOwner(),
      ],
      [
        "truncated data disables borrow power without breaking pricing",
        () => liquidity.truncate(),
      ],
    ];

    disablingFaults.forEach(([name, injectFault]) => {
      it(name, async () => {
        const priced = await loadPreparedBank(PRIMARY);
        await injectFault();
        const bank = await pulseAndFetch(priced);
        assert.equal(bank.cache.borrowPowerDisabled, 1);
      });
    });

    const invalidPricingInputs = [
      {
        name: "wrong Liquidity PDA fails with JuplendLiquidityValidationFailed",
        mutate: (pricing: PublicKey[]) => [
          ...pricing.slice(0, -1),
          Keypair.generate().publicKey,
        ],
        error: ERR_LIQUIDITY_VALIDATION,
      },
      {
        name: "missing Liquidity account fails with WrongNumberOfOracleAccounts",
        mutate: (pricing: PublicKey[]) => pricing.slice(0, -1),
        error: ERR_WRONG_NUMBER_OF_ORACLE_ACCOUNTS,
      },
    ];

    invalidPricingInputs.forEach(({ name, mutate, error }) => {
      it(name, async () => {
        const priced = await loadPreparedBank(PRIMARY);
        const tx = await pulseTx({
          ...priced,
          pricing: mutate(priced.pricing),
        });
        assertBankrunTxFailed(await txs.trySend(tx), error);
      });
    });
  });

  describe("lockdown gate", () => {
    SCENARIOS.forEach((scenario) => {
      it(`${scenario.name}: price unchanged across lock transitions`, async () => {
        const priced = await loadPreparedBank(scenario);

        await liquidity.setStatus(Status.ACTIVE);
        let bank = await pulseAndFetch(priced);
        const activePrice = cachedPrice(bank);
        assert.isTrue(
          activePrice.gt(0),
          `${scenario.name}: active price must be positive`
        );
        assert.equal(
          bank.cache.borrowPowerDisabled,
          0,
          `${scenario.name}: borrow power must be enabled while active`
        );

        for (const { label, apply, expectedDisabled } of LOCK_TRANSITIONS) {
          await apply();
          bank = await pulseAndFetch(priced);
          const price = cachedPrice(bank);
          assert.isTrue(
            isPriceUnchanged(price, activePrice),
            `${scenario.name} (${label}): cached price changed from ${activePrice} to ${price}`
          );
          assert.equal(
            bank.cache.borrowPowerDisabled,
            expectedDisabled,
            `${scenario.name} (${label}): unexpected borrowPowerDisabled value`
          );
        }
      });
    });
  });

  describe("borrow transaction enforcement during lockdown", () => {
    it(`${PRIMARY.name}: borrow fails while locked and succeeds when unlocked`, async () => {
      const market = await loadMarket();
      const borrower = new Actor(users[0], market.group);
      const borrowAmount = tokenB(1);
      let borrowOpened = false;

      try {
        await fundCollateral(borrower, market, usdc(1_000));

        let bank = await pulseWithStatus(market.collateral, Status.LOCKED);
        assert.equal(bank.cache.borrowPowerDisabled, 1);
        assertBankrunTxFailed(
          await borrower.tryBorrow(market.liabilityBankPk, borrowAmount),
          ERR_RISK_ENGINE_INIT_REJECTED
        );

        bank = await pulseWithStatus(market.collateral, Status.ACTIVE);
        assert.equal(bank.cache.borrowPowerDisabled, 0);
        await borrower.borrow(market.liabilityBankPk, borrowAmount);
        borrowOpened = true;
      } finally {
        liquidity.restore();
        if (borrowOpened) {
          await borrower.repayAll(
            market.liabilityBankPk,
            borrowAmount,
            borrowAmount.muln(2)
          );
        }
      }
    });
  });

  describe("other operations during lockdown", () => {
    it(`repay succeeds while ${PRIMARY.name} collateral is locked`, async () => {
      const market = await loadMarket();
      const borrower = new Actor(users[0], market.group);
      const borrowAmount = tokenB(1);

      try {
        await fundCollateral(borrower, market, usdc(1_000));
        await borrower.borrow(market.liabilityBankPk, borrowAmount);

        await pulseWithStatus(market.collateral, Status.LOCKED);
        await borrower.repayAll(
          market.liabilityBankPk,
          borrowAmount,
          borrowAmount.muln(2)
        );
      } finally {
        liquidity.restore();
      }
    });

    it("healthPulse succeeds while locked; initial health drops and maintenance health is unchanged", async () => {
      const market = await loadMarket();
      const borrower = new Actor(users[0], market.group);
      const borrowAmount = tokenB(1);

      try {
        await fundCollateral(borrower, market, usdc(1_000));
        await borrower.borrow(market.liabilityBankPk, borrowAmount);
        const active = await borrower.pulseHealth();

        await pulseWithStatus(market.collateral, Status.LOCKED);
        const locked = await borrower.pulseHealth();

        assert.isTrue(locked.initHealth.lt(active.initHealth));
        assert.isTrue(locked.maintHealth.eq(active.maintHealth));
        assert.notEqual(locked.flags & HEALTH_CACHE_ORACLE_OK, 0);
      } finally {
        liquidity.restore();
        await repayOrThrow("healthPulse", () =>
          borrower.repayAll(market.liabilityBankPk, borrowAmount)
        );
      }
    });

    it(`liquidation succeeds while ${PRIMARY.name} collateral is locked when liquidator has other collateral`, async () => {
      const market = await loadMarket();
      const borrower = new Actor(users[0], market.group);
      const liquidator = new Actor(users[1], market.group);
      const liquidatorUsdcBankPk = getRegisteredBank(
        JUPLEND_STATE_KEYS.jlr01RegularBankUsdc,
        "JLR01 regular USDC bank is missing"
      );
      const collateralAmount = usdc(1_000);
      const borrowAmount = tokenB(5);
      const liabilityWeightsBefore = (await fetchBank(market.liabilityBankPk))
        .config;

      try {
        await fundCollateral(borrower, market, collateralAmount);
        await borrower.borrow(market.liabilityBankPk, borrowAmount);

        await pulseWithStatus(market.collateral, Status.LOCKED);
        await setLiabilityWeights(
          market.group,
          market.liabilityBankPk,
          bigNumberToWrappedI80F48(new BigNumber(120)),
          bigNumberToWrappedI80F48(new BigNumber(100))
        );

        await liquidator.init();
        await liquidator.depositUsdc(liquidatorUsdcBankPk, collateralAmount);

        await liquidator.liquidate({
          assetBank: market.collateral.bankPk,
          liabilityBank: market.liabilityBankPk,
          liquidatee: borrower,
          amount: new BN(1_000),
          accounts: await buildLiquidationAccounts(
            market,
            liquidatorUsdcBankPk
          ),
        });
      } finally {
        liquidity.restore();
        await repayOrThrow("liquidation", () =>
          borrower.repayAll(market.liabilityBankPk, borrowAmount)
        );
        try {
          await setLiabilityWeights(
            market.group,
            market.liabilityBankPk,
            liabilityWeightsBefore.liabilityWeightInit,
            liabilityWeightsBefore.liabilityWeightMaint
          );
        } catch (error) {
          console.warn(
            "jlr14 cleanup (restore liability weights) failed:",
            error
          );
        }
      }
    });
  });
});
