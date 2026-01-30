import { AnchorProvider, BN, Program, Wallet } from "@coral-xyz/anchor";
import {
  createAssociatedTokenAccountInstruction,
  createInitializeMintInstruction,
  getAssociatedTokenAddressSync,
  MintLayout,
  TOKEN_PROGRAM_ID,
} from "@solana/spl-token";
import type { Connection, TransactionInstruction } from "@solana/web3.js";
import {
  Keypair,
  LAMPORTS_PER_SOL,
  PublicKey,
  SystemProgram,
  Transaction,
} from "@solana/web3.js";
import { Marginfi } from "../../target/types/marginfi";
import { Mocks } from "../../target/types/mocks";
import { KaminoLending } from "../fixtures/kamino_lending";
import { ProgramTestContext } from "solana-bankrun";
import { processBankrunTransaction } from "./tools";

export type Ecosystem = {
  /** A generic wsol mint with 9 decimals (same as native) */
  wsolMint: Keypair;
  /** A generic spl token mint */
  tokenAMint: Keypair;
  /** A generic spl token mint */
  tokenBMint: Keypair;
  /** A generic spl token mint like USDC (6 decimals) */
  usdcMint: Keypair;
  /** A generic LST-like mint (like wsol, 9 decimals) */
  lstAlphaMint: Keypair;
  /** 150 */
  wsolPrice: number;
  /** 9 */
  wsolDecimals: number;
  /** 10 */
  tokenAPrice: number;
  /** Decimals for token A (default 8) */
  tokenADecimals: number;
  /** 20 */
  tokenBPrice: number;
  /** Decimals for token B (default 6)*/
  tokenBDecimals: number;
  /** 1 */
  usdcPrice: number;
  /** 6 */
  usdcDecimals: number;
  /** 175 */
  lstAlphaPrice: number;
  /** Decimals for lst alpha (default 9)*/
  lstAlphaDecimals: number;
};

const WSOL_MINT_SEED = Buffer.from("WSOL_MINT_SEED_00000000000000000");
const TOKEN_A_MINT_SEED = Buffer.from("TOKEN_A_MINT_SEED_00000000000000");
const TOKEN_B_MINT_SEED = Buffer.from("TOKEN_B_MINT_SEED_00000000000000");
const USDC_MINT_SEED = Buffer.from("USDC_MINT_SEED_00000000000000002");
const LST_ALPHA_MINT_SEED = Buffer.from("LST_ALPHA_MINT_SEED_000000000000");

/**
 * Deterministic keypairs for all mints.
 * Determinism is necessary for the lending account balances order (based on sorting) to stay the same between different runs
 *
 * 6 Decimals for usdc. 9 decimals for sol. 8 decimals to token A, 6 for token B
 * @returns
 */
export const getGenericEcosystem = () => {
  const ecosystem: Ecosystem = {
    wsolMint: Keypair.fromSeed(WSOL_MINT_SEED),
    tokenAMint: Keypair.fromSeed(TOKEN_A_MINT_SEED),
    tokenBMint: Keypair.fromSeed(TOKEN_B_MINT_SEED),
    usdcMint: Keypair.fromSeed(USDC_MINT_SEED),
    lstAlphaMint: Keypair.fromSeed(LST_ALPHA_MINT_SEED),
    wsolPrice: 150,
    wsolDecimals: 9,
    tokenAPrice: 10,
    tokenADecimals: 8,
    tokenBPrice: 20,
    tokenBDecimals: 6,
    usdcPrice: 1,
    usdcDecimals: 6,
    lstAlphaPrice: 175,
    lstAlphaDecimals: 9,
  };
  return ecosystem;
};

/**
 * Print ecosystem info to console
 * @param ecosystem
 */
export const echoEcosystemInfo = (
  ecosystem: Ecosystem,
  {
    skipWsol = false,
    skipUsdc = false,
    skipA = false,
    skipB = false,
    skipAlpha = false,
  }
) => {
  if (!skipWsol) {
    console.log("wsol mint:........... " + ecosystem.wsolMint.publicKey);
    console.log("  wsol decimals...... " + ecosystem.wsolDecimals);
  }
  if (!skipUsdc) {
    console.log("usdc mint:........... " + ecosystem.usdcMint.publicKey);
    console.log("  usdc decimals:..... " + ecosystem.usdcDecimals);
  }
  if (!skipA) {
    console.log("token a mint:........ " + ecosystem.tokenAMint.publicKey);
    console.log("  token a decimals:.. " + ecosystem.tokenADecimals);
  }
  if (!skipB) {
    console.log("token b mint:........ " + ecosystem.tokenBMint.publicKey);
    console.log("  token b decimals:.. " + ecosystem.tokenBDecimals);
  }
  if (!skipAlpha) {
    console.log("lst alpha mint:...... " + ecosystem.lstAlphaMint.publicKey);
    console.log("  lst alpha decimals: " + ecosystem.lstAlphaDecimals);
  }
};

/**
 *  A typical user, with a wallet, ATAs for mock tokens, and a program to sign/send txes with.
 */
export type MockUser = {
  wallet: Keypair;
  /** Users's ATA for wsol*/
  wsolAccount: PublicKey;
  /** Users's ATA for token A */
  tokenAAccount: PublicKey;
  /** Users's ATA for token B */
  tokenBAccount: PublicKey;
  /** Users's ATA for USDC */
  usdcAccount: PublicKey;
  /** Users's ATA for LST Alpha */
  lstAlphaAccount: PublicKey;
  /** A program that uses the user's wallet */
  mrgnProgram: Program<Marginfi> | undefined;
  /** A bankrun program that uses the user's wallet */
  mrgnBankrunProgram: Program<Marginfi> | undefined;
  /** A Kamino bankrun program that uses the user's wallet */
  klendBankrunProgram: Program<KaminoLending> | undefined;
  /** A map to store arbitrary accounts related to the user using a string key */
  accounts: Map<string, PublicKey>;
};

/** in mockUser.accounts, key used to get/set the users's account for group 0 */
export const USER_ACCOUNT: string = "g0_acc";
/** in mockUser.accounts, key used to get/set the users's account for the emode group */
export const USER_ACCOUNT_E: string = "ge_acc";
/** in mockUser.accounts, key used to get/set the users's account for the kamino group */
export const USER_ACCOUNT_K: string = "gk_acc";
/** in mockUser.accounts, key used to get/set the users's account for the drift group */
export const USER_ACCOUNT_D: string = "gd_acc";
/** in mockUser.accounts, key used to get/set the users's account for the solend group */
export const USER_ACCOUNT_SL: string = "sl_acc";

/** in mockUser.accounts, key used to get/set the users's LST ATA for validator 0 */
export const LST_ATA = "v0_lstAta";
/** in mockUser.accounts, key used to get/set the users's LST stake account for validator 0 */
export const STAKE_ACC = "v0_stakeAcc";
/** in mockUser.accounts, the Kamino user metadata account */
export const KAMINO_METADATA = "kamino_metadata";
/** in mockUser.accounts, the obligation for the main market */
export const KAMINO_OBLIGATION = "kamino_obligation";
/** in mockUser.accounts, the Solend obligation account */
export const SOLEND_OBLIGATION = "solend_obligation";
/** in mockUser.accounts, key used for liquidation tests */
export const LIQ_TEST_ACCOUNT = "liq_test_account";
/** in mockUser.accounts, key used to get/set the users's LST ATA for validator 1 */
export const LST_ATA_v1 = "v1_lstAta";
/** in mockUser.accounts, key used to get/set the users's LST stake account for validator 1 */
export const STAKE_ACC_v1 = "v1_stakeAcc";

/**
 * Options to skip various parts of mock user setup
 */
export interface SetupTestUserOptions {
  marginProgram: Program<Marginfi>;
  /** Force the mock user to use this keypair */
  forceWallet: Keypair;
  wsolMint: PublicKey;
  tokenAMint: PublicKey;
  tokenBMint: PublicKey;
  usdcMint: PublicKey;
  lstAlphaMint: PublicKey;
}

/**
 * Creates and funds a user by transfering some SOL from a given wallet.
 *
 * Opens ATA for the user on all ecosystem mints
 *
 * Initializes a mock program to sign transactions as the user
 * @param provider
 * @param wallet - provider wallet, pays init and tx fees
 * @param options - skip parts of setup or force a keypair as the wallet
 * @returns
 */
export const setupTestUser = async (
  provider: AnchorProvider,
  wallet: Keypair,
  options?: SetupTestUserOptions
) => {
  // Creates a user wallet with some SOL in it to pay tx fees
  const userWalletKeypair = options.forceWallet || Keypair.generate();
  const userWallet = userWalletKeypair.publicKey;
  const tx: Transaction = new Transaction();
  tx.add(
    SystemProgram.transfer({
      fromPubkey: wallet.publicKey,
      toPubkey: userWallet,
      lamports: 1000 * LAMPORTS_PER_SOL,
    })
  );

  let wsolAccount: PublicKey = PublicKey.default;
  if (options.wsolMint) {
    wsolAccount = getAssociatedTokenAddressSync(options.wsolMint, userWallet);
    tx.add(
      createAssociatedTokenAccountInstruction(
        wallet.publicKey,
        wsolAccount,
        userWallet,
        options.wsolMint
      )
    );
  }

  let usdcAccount: PublicKey = PublicKey.default;
  if (options.usdcMint) {
    usdcAccount = getAssociatedTokenAddressSync(options.usdcMint, userWallet);
    tx.add(
      createAssociatedTokenAccountInstruction(
        wallet.publicKey,
        usdcAccount,
        userWallet,
        options.usdcMint
      )
    );
  }

  let tokenAAccount: PublicKey = PublicKey.default;
  if (options.tokenAMint) {
    tokenAAccount = getAssociatedTokenAddressSync(
      options.tokenAMint,
      userWallet
    );
    tx.add(
      createAssociatedTokenAccountInstruction(
        wallet.publicKey,
        tokenAAccount,
        userWallet,
        options.tokenAMint
      )
    );
  }

  let tokenBAccount: PublicKey = PublicKey.default;
  if (options.tokenBMint) {
    tokenBAccount = getAssociatedTokenAddressSync(
      options.tokenBMint,
      userWallet
    );
    tx.add(
      createAssociatedTokenAccountInstruction(
        wallet.publicKey,
        tokenBAccount,
        userWallet,
        options.tokenBMint
      )
    );
  }

  let alphaAccount: PublicKey = PublicKey.default;
  if (options.lstAlphaMint) {
    alphaAccount = getAssociatedTokenAddressSync(
      options.lstAlphaMint,
      userWallet
    );
    tx.add(
      createAssociatedTokenAccountInstruction(
        wallet.publicKey,
        alphaAccount,
        userWallet,
        options.lstAlphaMint
      )
    );
  }

  await provider.sendAndConfirm(tx, [wallet]);

  const user: MockUser = {
    wallet: userWalletKeypair,
    wsolAccount: wsolAccount,
    tokenAAccount: tokenAAccount,
    tokenBAccount: tokenBAccount,
    usdcAccount: usdcAccount,
    lstAlphaAccount: alphaAccount,

    mrgnProgram: options.marginProgram
      ? getUserMarginfiProgram(options.marginProgram, userWalletKeypair)
      : undefined,
    mrgnBankrunProgram: undefined,
    klendBankrunProgram: undefined,
    accounts: new Map<string, PublicKey>(),
  };
  return user;
};

/**
 * Bankrun-native version of setupTestUser.
 * Creates and funds a user using bankrun transactions (no local validator needed).
 */
export interface SetupTestUserBankrunOptions {
  wsolMint?: PublicKey;
  tokenAMint?: PublicKey;
  tokenBMint?: PublicKey;
  usdcMint?: PublicKey;
  lstAlphaMint?: PublicKey;
  forceWallet?: Keypair;
}

export const setupTestUserBankrun = async (
  bankrunContext: ProgramTestContext,
  payer: Keypair,
  options?: SetupTestUserBankrunOptions
): Promise<MockUser> => {
  const userWalletKeypair = options?.forceWallet || Keypair.generate();
  const userWallet = userWalletKeypair.publicKey;
  const tx: Transaction = new Transaction();

  // Fund user wallet with SOL
  tx.add(
    SystemProgram.transfer({
      fromPubkey: payer.publicKey,
      toPubkey: userWallet,
      lamports: 1000 * LAMPORTS_PER_SOL,
    })
  );

  let wsolAccount: PublicKey = PublicKey.default;
  if (options?.wsolMint) {
    wsolAccount = getAssociatedTokenAddressSync(options.wsolMint, userWallet);
    tx.add(
      createAssociatedTokenAccountInstruction(
        payer.publicKey,
        wsolAccount,
        userWallet,
        options.wsolMint
      )
    );
  }

  let usdcAccount: PublicKey = PublicKey.default;
  if (options?.usdcMint) {
    usdcAccount = getAssociatedTokenAddressSync(options.usdcMint, userWallet);
    tx.add(
      createAssociatedTokenAccountInstruction(
        payer.publicKey,
        usdcAccount,
        userWallet,
        options.usdcMint
      )
    );
  }

  let tokenAAccount: PublicKey = PublicKey.default;
  if (options?.tokenAMint) {
    tokenAAccount = getAssociatedTokenAddressSync(
      options.tokenAMint,
      userWallet
    );
    tx.add(
      createAssociatedTokenAccountInstruction(
        payer.publicKey,
        tokenAAccount,
        userWallet,
        options.tokenAMint
      )
    );
  }

  let tokenBAccount: PublicKey = PublicKey.default;
  if (options?.tokenBMint) {
    tokenBAccount = getAssociatedTokenAddressSync(
      options.tokenBMint,
      userWallet
    );
    tx.add(
      createAssociatedTokenAccountInstruction(
        payer.publicKey,
        tokenBAccount,
        userWallet,
        options.tokenBMint
      )
    );
  }

  let alphaAccount: PublicKey = PublicKey.default;
  if (options?.lstAlphaMint) {
    alphaAccount = getAssociatedTokenAddressSync(
      options.lstAlphaMint,
      userWallet
    );
    tx.add(
      createAssociatedTokenAccountInstruction(
        payer.publicKey,
        alphaAccount,
        userWallet,
        options.lstAlphaMint
      )
    );
  }

  // Process via bankrun
  await processBankrunTransaction(bankrunContext, tx, [payer]);

  const user: MockUser = {
    wallet: userWalletKeypair,
    wsolAccount: wsolAccount,
    tokenAAccount: tokenAAccount,
    tokenBAccount: tokenBAccount,
    usdcAccount: usdcAccount,
    lstAlphaAccount: alphaAccount,
    mrgnProgram: undefined,
    mrgnBankrunProgram: undefined,
    klendBankrunProgram: undefined,
    accounts: new Map<string, PublicKey>(),
  };
  return user;
};

/**
 * Bankrun-native mint creation.
 * Creates a mint using bankrun's rent sysvar for proper rent exemption calculation.
 */
export const createMintBankrun = async (
  bankrunContext: ProgramTestContext,
  payer: Keypair,
  decimals: number,
  mintKeypair: Keypair
): Promise<void> => {
  const rent = await bankrunContext.banksClient.getRent();
  const mintRentExemption = rent.minimumBalance(BigInt(MintLayout.span));

  const tx = new Transaction();
  tx.add(
    SystemProgram.createAccount({
      fromPubkey: payer.publicKey,
      newAccountPubkey: mintKeypair.publicKey,
      space: MintLayout.span,
      lamports: Number(mintRentExemption),
      programId: TOKEN_PROGRAM_ID,
    })
  );
  tx.add(
    createInitializeMintInstruction(
      mintKeypair.publicKey,
      decimals,
      payer.publicKey,
      payer.publicKey,
      TOKEN_PROGRAM_ID
    )
  );

  await processBankrunTransaction(bankrunContext, tx, [payer, mintKeypair]);
};

/**
 * Generates a mock program that can sign transactions as the user's wallet
 * @param program
 * @param userWallet
 * @returns
 */
export const getUserMarginfiProgram = (
  program: Program<Marginfi>,
  userWallet: Keypair | Wallet
): Program<Marginfi> => {
  const wallet =
    userWallet instanceof Keypair ? new Wallet(userWallet) : userWallet;
  const provider = new AnchorProvider(program.provider.connection, wallet, {});
  const userProgram = new Program<Marginfi>(program.idl, provider);
  return userProgram;
};

/**
 * Ixes to create a mint, the payer gains the Mint Tokens/Freeze authority
 * @param payer - pays account init fees, must sign, gains mint/freeze authority
 * @param provider
 * @param decimals
 * @param mintKeypair - (optional) generates random keypair if not provided, must sign
 * @param lamps - (optional) lamports to pay for created acc, fetches minimum for Mint exemption if
 * not provided
 * @returns ixes, and keypair of new mint
 */
export const createSimpleMint = async (
  payer: PublicKey,
  connection: Connection,
  decimals: number,
  mintKeypair?: Keypair,
  lamps?: number
) => {
  const mint = mintKeypair ? mintKeypair : Keypair.generate();
  const ixes: TransactionInstruction[] = [];
  const lamports = lamps
    ? lamps
    : await connection.getMinimumBalanceForRentExemption(MintLayout.span);
  ixes.push(
    SystemProgram.createAccount({
      fromPubkey: payer,
      newAccountPubkey: mint.publicKey,
      space: MintLayout.span,
      lamports: lamports,
      programId: TOKEN_PROGRAM_ID,
    })
  );
  ixes.push(
    createInitializeMintInstruction(
      mint.publicKey,
      decimals,
      payer,
      payer,
      TOKEN_PROGRAM_ID
    )
  );

  return { ixes, mint };
};

/**
 * Information about all the oracles in the world...
 *
 * If adding a Pyth Pull oracle with name *, make sure it is EXACTLY NAMED *Pull and
 * *PullOracleFeed, with price named *Price and *Decimals, the refresh all function searches for
 * those names exactly.
 */
export type Oracles = {
  wsolOracle: Keypair;
  wsolOracleFeed: Keypair;
  /** Default 150 */
  wsolPrice: number;
  wsolDecimals: number;
  usdcOracle: Keypair;
  usdcOracleFeed: Keypair;
  /** Default 1 */
  usdcPrice: number;
  usdcDecimals: number;
  tokenAOracle: Keypair;
  tokenAOracleFeed: Keypair;
  /** Default 10 */
  tokenAPrice: number;
  tokenADecimals: number;
  tokenBOracle: Keypair;
  tokenBOracleFeed: Keypair;
  /** Default 20 */
  tokenBPrice: number;
  tokenBDecimals: number;
  /** Default 175 */
  lstAlphaPrice: number;
  lstAlphaDecimals: number;
  /** Same initial price/decimals as USDC, but different key. */
  fakeUsdc: PublicKey;
  fakeUsdcFeed: PublicKey;
  /** Pyth pull oracle price feed that uses a SOL-like price and SOL decimals */
  pythPullLst: Keypair;
  /** the feed ID that pythPullLst oracle uses. */
  pythPullLstOracleFeed: Keypair;
};

/**
 * Creates an account to store data arbitrary data.
 * @param program - the mock program
 * @param space - for account space and rent exemption
 * @param wallet - pays tx fee
 * @param bankrunContext - pass if using bankrun, omit otherwise
 * @returns address of the newly created account
 */
export const createMockAccount = async (
  program: Program<Mocks>,
  space: number,
  wallet: Wallet,
  keypair?: Keypair,
  bankrunContext?: ProgramTestContext
) => {
  const newAccount = keypair ?? Keypair.generate();
  const createTx = new Transaction().add(
    SystemProgram.createAccount({
      fromPubkey: wallet.publicKey,
      newAccountPubkey: newAccount.publicKey,
      programId: program.programId,
      lamports:
        await program.provider.connection.getMinimumBalanceForRentExemption(
          space
        ),
      space,
    })
  );

  if (bankrunContext) {
    await processBankrunTransaction(bankrunContext, createTx, [
      wallet.payer,
      newAccount,
    ]);
  } else {
    await program.provider.sendAndConfirm(createTx, [wallet.payer, newAccount]);
  }
  return newAccount;
};

/**
 * Writes arbitrary bytes to a mock account
 * @param program - the Mock program
 * @param wallet - pays tx fee
 * @param account - account to write into (create with `createMockAccount` first)
 * @param offset - byte to start writing
 * @param input - bytes to write
 * @param bankrunContext - pass if using bankrun, omit otherwise
 */
export const storeMockAccount = async (
  program: Program<Mocks>,
  wallet: Wallet,
  account: Keypair,
  offset: number,
  input: Buffer,
  bankrunContext?: ProgramTestContext
) => {
  const tx = new Transaction().add(
    await program.methods
      .write(new BN(offset), input)
      .accounts({
        target: account.publicKey,
      })
      .instruction()
  );
  if (bankrunContext) {
    await processBankrunTransaction(bankrunContext, tx, [
      wallet.payer,
      account,
    ]);
  } else {
    await program.provider.sendAndConfirm(tx, [wallet.payer, account]);
  }
};

export type Validator = {
  node: PublicKey;
  authorizedVoter: PublicKey;
  authorizedWithdrawer: PublicKey;
  voteAccount: PublicKey;
  /** The spl stake pool itself, all PDAs derive from this key */
  splPool: PublicKey;
  /** spl pool's mint for the LST (a PDA automatically created on init) */
  splMint: PublicKey;
  /** spl pool's authority for LST management, a PDA with no data/lamports */
  splAuthority: PublicKey;
  /** spl pool's stake account (a PDA automatically created on init, contains the SOL held by the pool) */
  splSolPool: PublicKey;
  /** bank created for this validator's LST on the "main" group */
  bank: PublicKey;
};
