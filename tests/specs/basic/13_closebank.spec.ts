import { BN, Program } from "@coral-xyz/anchor";
import { BankrunProvider } from "../../utils/litesvm";
import { AccountMeta, PublicKey, Transaction } from "@solana/web3.js";
import { Marginfi } from "../../../target/types/marginfi";
import {
  BANK_ACCOUNT_LEN,
  bankKeypairA,
  bankKeypairUsdc,
  bankrunContext,
  bankrunProgram,
  bankRunProvider,
  ecosystem,
  groupAdmin,
  loadJsonFixture,
  marginfiGroup,
  oracles,
  users,
} from "../../rootHooks";
import {
  CLOSE_ENABLED_FLAG,
  defaultBankConfig,
  EMISSIONS_STREAM_MIN_SECONDS,
  ORACLE_SETUP_PYTH_PUSH,
} from "../../utils/types";
import {
  addBankWithSeed,
  lendingPoolEmissionsDepositStreamed,
} from "../../utils/group-instructions";
import {
  composeRemainingAccounts,
  depositIx,
  withdrawIx,
} from "../../utils/user-instructions";
import { deriveBankWithSeed, deriveLiquidityVault } from "../../utils/pdas";
import { assert } from "chai";
import { assertBNEqual, expectFailedTxWithError } from "../../utils/genericTests";
import { closeBank } from "../../utils/group-instructions";
import { USER_ACCOUNT } from "../../utils/mocks";
import { dumpAccBalances } from "../../utils/tools";
import { nativeToI80Scaled, toI80Scaled } from "../../utils/bn-utils";

let program: Program<Marginfi>;
let provider: BankrunProvider;

describe("Close bank", () => {
  let bankKey: PublicKey;
  const seed = new BN(987613);
  const oneTokenA = new BN(1 * 10 ** ecosystem.tokenADecimals);

  /** Adds a Token A bank under `bankSeed`, priced by the Token A oracle, and returns its key. */
  const addTokenABank = async (bankSeed: BN) => {
    const [bank] = deriveBankWithSeed(
      program.programId,
      marginfiGroup.publicKey,
      ecosystem.tokenAMint.publicKey,
      bankSeed
    );
    await groupAdmin.mrgnProgram.provider.sendAndConfirm(
      new Transaction().add(
        await addBankWithSeed(groupAdmin.mrgnProgram, {
          marginfiGroup: marginfiGroup.publicKey,
          feePayer: groupAdmin.wallet.publicKey,
          bankMint: ecosystem.tokenAMint.publicKey,
          config: defaultBankConfig(),
          seed: bankSeed,
        }),
        await program.methods
          .lendingPoolConfigureBankOracle(
            ORACLE_SETUP_PYTH_PUSH,
            oracles.tokenAOracle.publicKey
          )
          .accountsPartial({
            group: marginfiGroup.publicKey,
            bank,
            governanceAdmin: groupAdmin.wallet.publicKey,
          })
          .remainingAccounts([
            {
              pubkey: oracles.tokenAOracle.publicKey,
              isSigner: false,
              isWritable: false,
            } as AccountMeta,
          ])
          .instruction()
      )
    );
    return bank;
  };

  const depositInto = async (bank: PublicKey) => {
    await users[0].mrgnProgram.provider.sendAndConfirm(
      new Transaction().add(
        await depositIx(users[0].mrgnProgram, {
          marginfiAccount: users[0].accounts.get(USER_ACCOUNT),
          bank,
          tokenAccount: users[0].tokenAAccount,
          amount: oneTokenA,
          depositUpToLimit: false,
        })
      )
    );
  };

  const withdrawAllFrom = async (bank: PublicKey) => {
    // For withdrawAll, include all active balances, including the closing bank.
    const remaining = composeRemainingAccounts(
      [
        [bank, oracles.tokenAOracle.publicKey],
        [bankKeypairUsdc.publicKey, oracles.usdcOracle.publicKey],
        [bankKeypairA.publicKey, oracles.tokenAOracle.publicKey],
      ].filter((group) => !group[0].equals(bank))
    );
    await users[0].mrgnProgram.provider.sendAndConfirm(
      new Transaction().add(
        await withdrawIx(users[0].mrgnProgram, {
          marginfiAccount: users[0].accounts.get(USER_ACCOUNT),
          bank,
          tokenAccount: users[0].tokenAAccount,
          remaining,
          amount: new BN(0),
          withdrawAll: true,
        })
      )
    );
  };

  const expectCloseToFail = async (bank: PublicKey) => {
    await expectFailedTxWithError(
      async () => {
        await groupAdmin.mrgnProgram.provider.sendAndConfirm(
          new Transaction().add(
            await closeBank(groupAdmin.mrgnProgram, {
              marginfiGroup: marginfiGroup.publicKey,
              bank,
              admin: groupAdmin.wallet.publicKey,
            })
          )
        );
      },
      "BankCannotClose",
      6081
    );
  };

  before(async () => {
    provider = bankRunProvider;
    program = bankrunProgram;
    bankKey = await addTokenABank(seed);

    const bank = await program.account.bank.fetch(bankKey);
    assertBNEqual(bank.bankSeed, seed);
  });

  it("bank cannot close with open positions", async () => {
    await depositInto(bankKey);

    const bankAfterDeposit = await program.account.bank.fetch(bankKey);
    assert.equal(bankAfterDeposit.lendingPositionCount, 1);

    await expectCloseToFail(bankKey);
  });

  it("bank can be closed after the last user withdraws", async () => {
    const userAcc = users[0].accounts.get(USER_ACCOUNT);
    const acc = await users[0].mrgnProgram.account.marginfiAccount.fetch(
      userAcc
    );
    dumpAccBalances(acc);

    await withdrawAllFrom(bankKey);

    const bankAfterWithdraw = await program.account.bank.fetch(bankKey);
    assert.equal(bankAfterWithdraw.lendingPositionCount, 0);

    const groupBefore = await program.account.marginfiGroup.fetch(
      marginfiGroup.publicKey
    );
    await groupAdmin.mrgnProgram.provider.sendAndConfirm(
      new Transaction().add(
        await closeBank(groupAdmin.mrgnProgram, {
          marginfiGroup: marginfiGroup.publicKey,
          bank: bankKey,
          admin: groupAdmin.wallet.publicKey,
        })
      )
    );
    const groupAfter = await program.account.marginfiGroup.fetch(
      marginfiGroup.publicKey
    );
    assert.equal(groupAfter.banks, groupBefore.banks - 1);

    const info = await provider.connection.getAccountInfo(bankKey);
    assert.isNull(info);
  });

  it("bank cannot close with emissions left to pay", async () => {
    const streamingBank = await addTokenABank(seed.addn(1));
    await depositInto(streamingBank);
    const [liquidityVault] = deriveLiquidityVault(
      program.programId,
      streamingBank
    );
    await users[0].mrgnProgram.provider.sendAndConfirm(
      new Transaction().add(
        await lendingPoolEmissionsDepositStreamed(users[0].mrgnProgram, {
          bank: streamingBank,
          mint: ecosystem.tokenAMint.publicKey,
          fundingAccount: users[0].tokenAAccount,
          depositor: users[0].wallet.publicKey,
          liquidityVault,
          amount: oneTokenA,
          seconds: new BN(EMISSIONS_STREAM_MIN_SECONDS),
        })
      )
    );

    // Withdrawn in the same second, the stream has paid nothing and has no one left to pay.
    await withdrawAllFrom(streamingBank);
    const bank = await program.account.bank.fetch(streamingBank);
    assert.equal(bank.lendingPositionCount, 0);
    assert.equal(toI80Scaled(bank.totalAssetShares), 0n);
    assert.equal(
      toI80Scaled(bank.emissionsStreamPending),
      nativeToI80Scaled(oneTokenA)
    );
    await expectCloseToFail(streamingBank);
  });

  // Uses a real mainnet fixture: the legacy staked bank Hco1P3dGRXz3ZGFvMkbDgghZQy47Tp7vp7koSYRvP6nm
  // (MRGN3). It predates 0.1.4, so CLOSE_ENABLED_FLAG is unset and a normal close is rejected — but
  // it holds zero shares/emissions, so it can be force-closed. The fixture's `group` field is
  // re-pointed to the test group (whose admin is `groupAdmin`) during fixture prep.
  describe("force_close", () => {
    const FORCE_BANK = new PublicKey(
      "Hco1P3dGRXz3ZGFvMkbDgghZQy47Tp7vp7koSYRvP6nm"
    );

    before(() => {
      const fixture = loadJsonFixture(
        "tests/fixtures/mainnet_force_close_bank.json",
        BANK_ACCOUNT_LEN
      );
      bankrunContext.setAccount(fixture.address, fixture.info);
    });

    it("rejects a normal close (CLOSE_ENABLED_FLAG unset)", async () => {
      const bank = await program.account.bank.fetch(FORCE_BANK);
      assert.equal(bank.flags.toNumber() & CLOSE_ENABLED_FLAG, 0);

      await expectFailedTxWithError(
        async () => {
          await groupAdmin.mrgnProgram.provider.sendAndConfirm(
            new Transaction().add(
              await closeBank(groupAdmin.mrgnProgram, {
                marginfiGroup: marginfiGroup.publicKey,
                bank: FORCE_BANK,
                admin: groupAdmin.wallet.publicKey,
              })
            )
          );
        },
        "BankCannotClose",
        6081
      );

      assert.isNotNull(await provider.connection.getAccountInfo(FORCE_BANK));
    });

    it("closes with force_close = true", async () => {
      const groupBefore = await program.account.marginfiGroup.fetch(
        marginfiGroup.publicKey
      );
      await groupAdmin.mrgnProgram.provider.sendAndConfirm(
        new Transaction().add(
          await closeBank(groupAdmin.mrgnProgram, {
            marginfiGroup: marginfiGroup.publicKey,
            bank: FORCE_BANK,
            forceClose: true,
            admin: groupAdmin.wallet.publicKey,
          })
        )
      );
      const groupAfter = await program.account.marginfiGroup.fetch(
        marginfiGroup.publicKey
      );
      assert.equal(groupAfter.banks, groupBefore.banks - 1);
      assert.isNull(await provider.connection.getAccountInfo(FORCE_BANK));
    });
  });
});
