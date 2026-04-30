import { BN, Program } from "@coral-xyz/anchor";
import { Clock } from "solana-bankrun";
import { Transaction } from "@solana/web3.js";
import { Marginfi } from "../target/types/marginfi";
import {
  bankKeypairSol,
  bankrunProgram,
  ecosystem,
  groupAdmin,
  marginfiGroup,
  oracles,
  users,
} from "./rootHooks";
import {
  assertBankrunTxFailed,
  assertBNEqual,
} from "./utils/genericTests";
import {
  BankConfigOptRaw,
  blankBankConfigOptRaw,
  CIRCUIT_BREAKER_ENABLED,
} from "./utils/types";
import {
  clearCircuitBreaker,
  configureBank,
  groupConfigure,
} from "./utils/group-instructions";
import { pulseBankPrice } from "./utils/user-instructions";
import { setPythPullOraclePrice } from "./utils/bankrun-oracles";
import { bankrunContext, banksClient } from "./rootHooks";
import { getBankrunBlockhash } from "./utils/tools";
import { assert } from "chai";

describe("Circuit breaker config + admin clear", () => {
  let program: Program<Marginfi>;
  // SOL bank is not frozen by 04_configureBank (USDC is), so configureBank actually applies here.
  const bankKey = bankKeypairSol.publicKey;

  // Reasonable defaults: 5% / 10% / 25% tiers, 10m / 1h / 4h durations, 3-slot sustain,
  // 2x escalation window, α=0.1.
  const validCbOpts = (): BankConfigOptRaw => ({
    ...blankBankConfigOptRaw(),
    circuitBreakerEnabled: true,
    cbDeviationBpsTiers: [500, 1000, 2500],
    cbTierDurationsSeconds: [600, 3600, 14400],
    cbSustainObservations: 3,
    cbEscalationWindowMult: 2,
    cbEmaAlphaBps: 1000,
  });

  // CB enable now requires the cached price to be no older than CB_ENABLE_MAX_PRICE_AGE_SECONDS
  // (30s), so each enable test must be preceded by a pulse. This helper lands a fresh pulse on
  // the SOL bank. It first warps a few slots so each call produces a distinct blockhash —
  // bankrun's signature cache otherwise rejects identical pulse-tx bodies as "already processed"
  // when failing configure-bank txs in between don't advance the slot.
  const freshPulse = async () => {
    const clock = await banksClient.getClock();
    bankrunContext.warpToSlot(clock.slot + 5n);
    const tx = new Transaction().add(
      await pulseBankPrice(bankrunProgram, {
        group: marginfiGroup.publicKey,
        bank: bankKey,
        remaining: [oracles.wsolOracle.publicKey],
      })
    );
    tx.recentBlockhash = await getBankrunBlockhash(bankrunContext);
    tx.sign(groupAdmin.wallet);
    await banksClient.processTransaction(tx);
  };

  before(async () => {
    program = bankrunProgram;
    // Group's risk_admin defaults to Pubkey::default() at init. Set it to groupAdmin so the
    // clear-halt ix accepts it as an authority. (admin is also accepted, but the spike test
    // below explicitly exercises the risk_admin path.) Idempotent if already set.
    const group = await program.account.marginfiGroup.fetch(marginfiGroup.publicKey);
    if (!group.riskAdmin.equals(groupAdmin.wallet.publicKey)) {
      const tx = new Transaction().add(
        await groupConfigure(groupAdmin.mrgnBankrunProgram, {
          marginfiGroup: marginfiGroup.publicKey,
          newRiskAdmin: groupAdmin.wallet.publicKey,
        })
      );
      tx.recentBlockhash = await getBankrunBlockhash(bankrunContext);
      tx.sign(groupAdmin.wallet);
      await banksClient.processTransaction(tx);
    }
  });

  it("(admin) enabling CB with invalid config fails", async () => {
    await freshPulse();
    // sustain_observations = 0 is not usable — validate_circuit_breaker rejects.
    const bad: BankConfigOptRaw = {
      ...validCbOpts(),
      cbSustainObservations: 0,
    };
    const tx = new Transaction().add(
      await configureBank(groupAdmin.mrgnBankrunProgram, {
        bank: bankKey,
        bankConfigOpt: bad,
      })
    );
    tx.recentBlockhash = await getBankrunBlockhash(bankrunContext);
    tx.sign(groupAdmin.wallet);
    const result = await banksClient.tryProcessTransaction(tx);
    // CircuitBreakerInvalidConfig = 6602
    assertBankrunTxFailed(result, 6602);
  });

  it("(admin) enabling CB with non-monotonic tier thresholds fails", async () => {
    await freshPulse();
    const bad: BankConfigOptRaw = {
      ...validCbOpts(),
      cbDeviationBpsTiers: [500, 400, 2500], // tier 2 lower than tier 1
    };
    const tx = new Transaction().add(
      await configureBank(groupAdmin.mrgnBankrunProgram, {
        bank: bankKey,
        bankConfigOpt: bad,
      })
    );
    tx.recentBlockhash = await getBankrunBlockhash(bankrunContext);
    tx.sign(groupAdmin.wallet);
    const result = await banksClient.tryProcessTransaction(tx);
    assertBankrunTxFailed(result, 6602);
  });

  it("(admin) enabling CB with non-contiguous tiers fails", async () => {
    await freshPulse();
    const bad: BankConfigOptRaw = {
      ...validCbOpts(),
      cbDeviationBpsTiers: [500, 0, 2500],
      cbTierDurationsSeconds: [600, 0, 14400],
    };
    const tx = new Transaction().add(
      await configureBank(groupAdmin.mrgnBankrunProgram, {
        bank: bankKey,
        bankConfigOpt: bad,
      })
    );
    tx.recentBlockhash = await getBankrunBlockhash(bankrunContext);
    tx.sign(groupAdmin.wallet);
    const result = await banksClient.tryProcessTransaction(tx);
    assertBankrunTxFailed(result, 6602);
  });

  it("(admin) enabling CB with missing tier 3 fails", async () => {
    await freshPulse();
    const bad: BankConfigOptRaw = {
      ...validCbOpts(),
      cbDeviationBpsTiers: [500, 1000, 0],
      cbTierDurationsSeconds: [600, 3600, 0],
    };
    const tx = new Transaction().add(
      await configureBank(groupAdmin.mrgnBankrunProgram, {
        bank: bankKey,
        bankConfigOpt: bad,
      })
    );
    tx.recentBlockhash = await getBankrunBlockhash(bankrunContext);
    tx.sign(groupAdmin.wallet);
    const result = await banksClient.tryProcessTransaction(tx);
    assertBankrunTxFailed(result, 6602);
  });

  it("(admin) enable CB with valid config - happy path", async () => {
    await freshPulse();
    const tx = new Transaction().add(
      await configureBank(groupAdmin.mrgnBankrunProgram, {
        bank: bankKey,
        bankConfigOpt: validCbOpts(),
      })
    );
    tx.recentBlockhash = await getBankrunBlockhash(bankrunContext);
    tx.sign(groupAdmin.wallet);
    await banksClient.processTransaction(tx);

    const bank = await program.account.bank.fetch(bankKey);
    assert.equal(
      (Number(bank.flags) & CIRCUIT_BREAKER_ENABLED) === CIRCUIT_BREAKER_ENABLED,
      true,
      "CIRCUIT_BREAKER_ENABLED should be set"
    );
    assert.deepEqual(bank.config.cbDeviationBpsTiers, [500, 1000, 2500]);
    assert.deepEqual(bank.config.cbTierDurationsSeconds, [600, 3600, 14400]);
    assert.equal(bank.config.cbSustainObservations, 3);
    assert.equal(bank.config.cbEscalationWindowMult, 2);
    assert.equal(bank.config.cbEmaAlphaBps, 1000);
    assert.equal(bank.cbTier, 0);
    assertBNEqual(bank.cbHaltStartedAt, 0);
    assertBNEqual(bank.cbHaltEndedAt, 0);
  });

  it("(non-admin) clear_circuit_breaker fails with Unauthorized", async () => {
    const attacker = users[0];
    const tx = new Transaction().add(
      await clearCircuitBreaker(attacker.mrgnBankrunProgram, { bank: bankKey })
    );
    tx.recentBlockhash = await getBankrunBlockhash(bankrunContext);
    tx.sign(attacker.wallet);
    const result = await banksClient.tryProcessTransaction(tx);
    // Unauthorized = 6042
    assertBankrunTxFailed(result, 6042);
  });

  it("(risk_admin) clear_circuit_breaker - happy path (no active halt is a no-op)", async () => {
    // The group's risk_admin defaults to groupAdmin in rootHooks setup (no separate risk_admin set).
    const tx = new Transaction().add(
      await clearCircuitBreaker(groupAdmin.mrgnBankrunProgram, { bank: bankKey })
    );
    tx.recentBlockhash = await getBankrunBlockhash(bankrunContext);
    tx.sign(groupAdmin.wallet);
    await banksClient.processTransaction(tx);

    const bank = await program.account.bank.fetch(bankKey);
    assert.equal(bank.cbTier, 0);
    assertBNEqual(bank.cbHaltStartedAt, 0);
    assertBNEqual(bank.cbHaltEndedAt, 0);
    assert.equal(bank.cache.cbBreachCount, 0);
  });

  // Walk slot + clock + Pyth price + pulse three times to force a sustained breach.
  // - `warpToSlot(+5)` clears `CB_MIN_PULSE_SLOT_GAP` (= 2) and advances bankrun's internal
  //   block state so each tx gets a fresh blockhash.
  // - `setClock` then explicitly bumps `unix_timestamp` by 1s, since `warpToSlot` alone doesn't
  //   reliably advance it. The CB's source-time dedup requires a strictly-advancing publish_time
  //   (which `setPythPullOraclePrice` derives from the current clock) — without this bump only
  //   the first pulse would count.
  const driveSustainedBreachToSpike = async (uiPrice: number) => {
    for (let i = 0; i < 3; i++) {
      const pre = await banksClient.getClock();
      bankrunContext.warpToSlot(pre.slot + 5n);
      const post = await banksClient.getClock();
      bankrunContext.setClock(
        new Clock(
          post.slot,
          post.epochStartTimestamp,
          post.epoch,
          post.leaderScheduleEpoch,
          post.unixTimestamp + 1n,
        )
      );
      await setPythPullOraclePrice(
        bankrunContext,
        banksClient,
        oracles.wsolOracle.publicKey,
        oracles.wsolOracleFeed.publicKey,
        uiPrice,
        ecosystem.wsolDecimals,
        0, // confidence interval — keep at 0 so the CB sees the full raw delta
      );
      const tx = new Transaction().add(
        await pulseBankPrice(bankrunProgram, {
          group: marginfiGroup.publicKey,
          bank: bankKey,
          remaining: [oracles.wsolOracle.publicKey],
        })
      );
      tx.recentBlockhash = await getBankrunBlockhash(bankrunContext);
      tx.sign(groupAdmin.wallet);
      await banksClient.processTransaction(tx);
    }
  };

  it("(real spike) tier-1 trip → escalation watch → tier-2 escalation → admin clear", async () => {
    // SOL bank starts operational with `cb_reference_price` seeded at $150 (the value enabled
    // earlier in the happy-path test). With α=10%, tiers=[500,1000,2500] bps, sustain=3:
    //   $162 → dev decays 800 → 714 → 638 bps (all tier 1, since 800 < 1000) → trips to tier 1.
    //   $170 inside the escalation window → dev ~1100 → 1035 → 922 bps; the first two cross
    //   tier 2 (>= 1000) and `cb_max_breached_tier_in_streak` records 2, but the escalation rule
    //   `new_tier = (cb_tier+1).min(3)` bumps from tier 1 → 2 anyway.
    // Note: $165 (the prior choice) lands at exactly 1000 bps on the first observation, which
    // crosses the tier-2 threshold and makes the post-trip tier 2 instead of 1.
    const before = await bankrunProgram.account.bank.fetch(bankKey);
    assert.equal(before.cbTier, 0);

    // ---- Stage 1: drive 3 sustained $162 pulses → tier-1 trip.
    await driveSustainedBreachToSpike(162);
    const afterTrip1 = await bankrunProgram.account.bank.fetch(bankKey);
    assert.equal(afterTrip1.cbTier, 1, "3 sustained pulses at $162 must trip tier 1");
    const haltEnded1 = afterTrip1.cbHaltEndedAt.toNumber();
    assert.isAbove(haltEnded1, 0);

    // ---- Stage 2: advance past halt_ended_at into the escalation window. is_cb_halted goes
    // false but cb_tier stays at 1 — a sustained re-breach inside the window will escalate.
    // warpToSlot advances slot + derived unix_time; setClock then pins unix_time to just past
    // halt_ended_at without rewinding the slot.
    {
      const clock = await banksClient.getClock();
      bankrunContext.warpToSlot(clock.slot + 1500n);
      const post = await banksClient.getClock();
      bankrunContext.setClock(
        new Clock(
          post.slot,
          post.epochStartTimestamp,
          post.epoch,
          post.leaderScheduleEpoch,
          BigInt(haltEnded1 + 10),
        )
      );
    }

    // ---- Stage 3: 3 more sustained pulses at $170 → escalation to tier 2.
    await driveSustainedBreachToSpike(170);
    const afterTrip2 = await bankrunProgram.account.bank.fetch(bankKey);
    assert.equal(
      afterTrip2.cbTier,
      2,
      "re-breach inside escalation window must escalate cb_tier from 1 → 2"
    );

    // ---- Stage 4: risk admin clears with reseedReference=true so the next pulse reseeds the
    // EMA from the (still-spiked) live oracle.
    const clearTx = new Transaction().add(
      await clearCircuitBreaker(groupAdmin.mrgnBankrunProgram, {
        bank: bankKey,
        reseedReference: true,
      })
    );
    clearTx.recentBlockhash = await getBankrunBlockhash(bankrunContext);
    clearTx.sign(groupAdmin.wallet);
    await banksClient.processTransaction(clearTx);

    const cleared = await bankrunProgram.account.bank.fetch(bankKey);
    assert.equal(cleared.cbTier, 0);
    assertBNEqual(cleared.cbHaltStartedAt, 0);
    assertBNEqual(cleared.cbHaltEndedAt, 0);
    assert.equal(cleared.cbTier3ConsecutiveTrips, 0);
  });

  it("(admin) disable CB cleanly", async () => {
    const tx = new Transaction().add(
      await configureBank(groupAdmin.mrgnBankrunProgram, {
        bank: bankKey,
        bankConfigOpt: {
          ...blankBankConfigOptRaw(),
          circuitBreakerEnabled: false,
        },
      })
    );
    tx.recentBlockhash = await getBankrunBlockhash(bankrunContext);
    tx.sign(groupAdmin.wallet);
    await banksClient.processTransaction(tx);

    const bank = await program.account.bank.fetch(bankKey);
    assert.equal(
      (Number(bank.flags) & CIRCUIT_BREAKER_ENABLED) === 0,
      true,
      "CIRCUIT_BREAKER_ENABLED should be unset"
    );
  });
});
