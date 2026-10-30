import { BN, Program } from "@coral-xyz/anchor";
import { BankrunProvider } from "../../utils/litesvm";
import {
  AccountMeta,
  PublicKey,
  Transaction,
  TransactionInstruction,
} from "@solana/web3.js";
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
  ORACLE_SETUP_PYTH_PUSH,
} from "../../utils/types";
import { addBankWithSeed } from "../../utils/group-instructions";
import {
  composeRemainingAccounts,
  depositIx,
  withdrawIx,
} from "../../utils/user-instructions";
import {
  deriveBankWithSeed,
  deriveFeeVault,
  deriveInsuranceVault,
  deriveLiquidityVault,
} from "../../utils/pdas";
import {
  createTransferInstruction,
  TOKEN_PROGRAM_ID,
} from "@solana/spl-token";
import { assert } from "chai";
import { bigNumberToWrappedI80F48 } from "@mrgnlabs/mrgn-common";
import {
  assertBNEqual,
  assertI80F48Equal,
  expectFailedTxWithError,
  expectFailedTxWithMessage,
  getTokenBalance,
} from "../../utils/genericTests";
import { closeBank } from "../../utils/group-instructions";
import { USER_ACCOUNT } from "../../utils/mocks";
import { dumpAccBalances } from "../../utils/tools";

let program: Program<Marginfi>;
let provider: BankrunProvider;

describe("Close bank", () => {
  let bankKey: PublicKey;
  const seed = new BN(987613);

  before(async () => {
    provider = bankRunProvider;
    program = bankrunProgram;
    const config = defaultBankConfig();
    [bankKey] = deriveBankWithSeed(
      program.programId,
      marginfiGroup.publicKey,
      ecosystem.tokenAMint.publicKey,
      seed
    );
    await groupAdmin.mrgnProgram.provider.sendAndConfirm(
      new Transaction().add(
        await addBankWithSeed(groupAdmin.mrgnProgram, {
          marginfiGroup: marginfiGroup.publicKey,
          feePayer: groupAdmin.wallet.publicKey,
          bankMint: ecosystem.tokenAMint.publicKey,
          config: config,
          seed: seed,
        }),
        await program.methods
          .lendingPoolConfigureBankOracle(
            ORACLE_SETUP_PYTH_PUSH,
            oracles.tokenAOracle.publicKey
          )
          .accountsPartial({
            group: marginfiGroup.publicKey,
            bank: bankKey,
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

    const bank = await program.account.bank.fetch(bankKey);
    assertBNEqual(bank.bankSeed, seed);
  });

  it("bank cannot close with open positions", async () => {
    const userAcc = users[0].accounts.get(USER_ACCOUNT);
    const amount = new BN(1 * 10 ** ecosystem.tokenADecimals);
    await users[0].mrgnProgram.provider.sendAndConfirm(
      new Transaction().add(
        await depositIx(users[0].mrgnProgram, {
          marginfiAccount: userAcc,
          bank: bankKey,
          tokenAccount: users[0].tokenAAccount,
          amount: amount,
          depositUpToLimit: false,
        })
      )
    );

    const bankAfterDeposit = await program.account.bank.fetch(bankKey);
    assert.equal(bankAfterDeposit.lendingPositionCount, 1);

    await expectFailedTxWithError(
      async () => {
        await groupAdmin.mrgnProgram.provider.sendAndConfirm(
          new Transaction().add(
            await closeBank(groupAdmin.mrgnProgram, {
              marginfiGroup: marginfiGroup.publicKey,
              bank: bankKey,
              admin: groupAdmin.wallet.publicKey,
            })
          )
        );
      },
      "BankCannotClose",
      6081
    );
  });

  it("bank can be closed after the last user withdraws", async () => {
    const userAcc = users[0].accounts.get(USER_ACCOUNT);
    const acc = await users[0].mrgnProgram.account.marginfiAccount.fetch(
      userAcc
    );
    dumpAccBalances(acc);

    // For withdrawAll, include all active balances, including the closing bank.
    const remaining = composeRemainingAccounts(
      [
        [bankKey, oracles.tokenAOracle.publicKey],
        [bankKeypairUsdc.publicKey, oracles.usdcOracle.publicKey],
        [bankKeypairA.publicKey, oracles.tokenAOracle.publicKey],
      ].filter((group) => !group[0].equals(bankKey))
    );
    await users[0].mrgnProgram.provider.sendAndConfirm(
      new Transaction().add(
        await withdrawIx(users[0].mrgnProgram, {
          marginfiAccount: userAcc,
          bank: bankKey,
          tokenAccount: users[0].tokenAAccount,
          remaining,
          amount: new BN(0),
          withdrawAll: true,
        })
      )
    );

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

  describe("closing the vaults", () => {
    const vaultSeed = new BN(987614);
    let vaultBank: PublicKey;
    let vaults: PublicKey[];

    before(async () => {
      [vaultBank] = deriveBankWithSeed(
        program.programId,
        marginfiGroup.publicKey,
        ecosystem.tokenAMint.publicKey,
        vaultSeed
      );
      await groupAdmin.mrgnProgram.provider.sendAndConfirm(
        new Transaction().add(
          await addBankWithSeed(groupAdmin.mrgnProgram, {
            marginfiGroup: marginfiGroup.publicKey,
            feePayer: groupAdmin.wallet.publicKey,
            bankMint: ecosystem.tokenAMint.publicKey,
            config: defaultBankConfig(),
            seed: vaultSeed,
          })
        )
      );
      vaults = [deriveLiquidityVault, deriveInsuranceVault, deriveFeeVault].map(
        (derive) => derive(program.programId, vaultBank)[0]
      );
    });

    const closeIx = (adminTokenAccount = groupAdmin.tokenAAccount) =>
      closeBank(groupAdmin.mrgnProgram, {
        marginfiGroup: marginfiGroup.publicKey,
        bank: vaultBank,
        admin: groupAdmin.wallet.publicKey,
        closeVaults: {
          tokenProgram: TOKEN_PROGRAM_ID,
          mint: ecosystem.tokenAMint.publicKey,
          adminTokenAccount,
        },
      });
    const send = async (ix: TransactionInstruction) =>
      groupAdmin.mrgnProgram.provider.sendAndConfirm(new Transaction().add(ix));

    it("rejects a partial vault list", async () => {
      const ix = await closeIx();
      ix.keys = ix.keys.slice(0, ix.keys.length - 2); // drop the fee vault and its authority
      await expectFailedTxWithMessage(
        () => send(ix),
        "expected at least 9 close accounts, got 7"
      );
    });

    it("rejects an admin token account the admin doesn't own", async () => {
      await expectFailedTxWithMessage(
        async () => send(await closeIx(users[0].tokenAAccount)),
        "admin_token_account must be owned by the admin"
      );
    });

    // Byte offsets: the coder can't re-encode a bank, so patch it in place.
    const owedToProtocol = [
      { name: "program fees are", field: "collectedProgramFeesOutstanding", offset: 904 },
      { name: "premium is", field: "collectedPremiumOutstanding", offset: 1736 },
    ] as const;
    for (const { name, field, offset } of owedToProtocol) {
      it(`rejects while ${name} uncollected`, async () => {
        const original = await provider.connection.getAccountInfo(vaultBank);
        const data = Buffer.from(original.data);
        data.set(bigNumberToWrappedI80F48(1).value, offset);
        bankrunContext.setAccount(vaultBank, { ...original, data });
        const bank = await program.account.bank.fetch(vaultBank);
        assertI80F48Equal(bank[field], 1);

        await expectFailedTxWithMessage(
          async () => send(await closeIx()),
          "collect the program fees and premium before closing the vaults"
        );
        bankrunContext.setAccount(vaultBank, original);
      });
    }

    it("sweeps donations to the admin, closes the bank and all three vaults", async () => {
      const donations = [1, 2, 3];
      await users[0].mrgnProgram.provider.sendAndConfirm(
        new Transaction().add(
          ...vaults.map((vault, i) =>
            createTransferInstruction(
              users[0].tokenAAccount,
              vault,
              users[0].wallet.publicKey,
              donations[i]
            )
          )
        )
      );

      const accounts = [vaultBank, ...vaults];
      const rent = (
        await Promise.all(
          accounts.map((k) => provider.connection.getAccountInfo(k))
        )
      ).reduce((sum, info) => sum + info.lamports, 0);
      const [adminBefore, tokensBefore] = await Promise.all([
        provider.connection.getBalance(groupAdmin.wallet.publicKey),
        getTokenBalance(provider, groupAdmin.tokenAAccount),
      ]);

      await send(await closeIx());

      for (const k of accounts) {
        assert.isNull(await provider.connection.getAccountInfo(k));
      }
      const [adminAfter, tokensAfter] = await Promise.all([
        provider.connection.getBalance(groupAdmin.wallet.publicKey),
        getTokenBalance(provider, groupAdmin.tokenAAccount),
      ]);
      assert.equal(tokensAfter - tokensBefore, 6);
      // Minus the tx fee.
      assert.isAtLeast(adminAfter - adminBefore, rent - 10_000);
    });
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
