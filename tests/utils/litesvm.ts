import { Wallet } from "@coral-xyz/anchor";
import { utils } from "@coral-xyz/anchor";
import {
  AccountInfo,
  Keypair,
  PublicKey,
  SendTransactionError,
  Transaction,
  VersionedTransaction,
} from "@solana/web3.js";
import { LiteSVMProvider as AnchorLiteSVMProvider } from "anchor-litesvm";
import {
  Clock,
  FailedTransactionMetadata,
  LiteSVM,
  TransactionMetadata,
} from "anchor-litesvm/node_modules/litesvm";
import fs from "fs";
import path from "path";

export { Clock };

export type AddedProgram = {
  name: string;
  programId: PublicKey;
};

export type AddedAccount = {
  address: PublicKey;
  info: AccountInfo<Buffer>;
};

export type LiteSVMTransactionMeta = {
  logMessages: string[];
  computeUnitsConsumed: bigint;
};

export type LiteSVMTransactionResultWithMeta = {
  result: string | null;
  meta: LiteSVMTransactionMeta;
};

export type BanksTransactionMeta = LiteSVMTransactionMeta;
export type BanksTransactionResultWithMeta = LiteSVMTransactionResultWithMeta;

const LOCAL_PROGRAMS: AddedProgram[] = [
  {
    name: "marginfi",
    programId: new PublicKey("2jGhuVUuy3umdzByFx8sNWUAaf5vaeuDm78RDPEnhrMr"),
  },
  {
    name: "mocks",
    programId: new PublicKey("rec5EKMGg6MxZYaMdyBfgwp4d5rB9T1VQH5pJv5LtFJ"),
  },
  {
    name: "test_transfer_hook",
    programId: new PublicKey("TRANSFERHKTRANSFERHKTRANSFERHKTRANSFERHKTRA"),
  },
];

const PAYER_LAMPORTS = 1_000_000_000_000_000n;
const INITIAL_SLOT = 1n;
const INITIAL_UNIX_TIMESTAMP = 1_700_000_000n;

const asBufferAccount = (
  account: AccountInfo<Uint8Array> | null,
): AccountInfo<Buffer> | null => {
  if (!account) return null;
  return {
    ...account,
    data: Buffer.from(account.data),
  };
};

const MAX_U64 = 18_446_744_073_709_551_615n;

const normalizeRentEpoch = (rentEpoch: number | bigint): bigint => {
  if (typeof rentEpoch === "bigint") return rentEpoch;
  if (!Number.isSafeInteger(rentEpoch) && rentEpoch >= Number.MAX_SAFE_INTEGER) {
    return MAX_U64;
  }
  return BigInt(rentEpoch);
};

const normalizeAccount = (
  account: AccountInfo<Buffer>,
): AccountInfo<Uint8Array> => ({
  ...account,
  data: new Uint8Array(account.data),
  rentEpoch: normalizeRentEpoch(account.rentEpoch),
});

const syncRecentSlotHash = (svm: LiteSVM): void => {
  const hashes = svm.getSlotHashes();
  if (hashes.length === 0) return;

  const current = hashes[0];
  current.slot = svm.getClock().slot;
  current.hash = svm.latestBlockhash();
  svm.setSlotHashes([
    current,
    ...hashes.slice(1).filter((hash) => hash.slot !== current.slot),
  ]);
};

const txSignature = (tx: Transaction | VersionedTransaction): string => {
  const signature =
    tx instanceof Transaction ? tx.signature : tx.signatures[0] ?? null;
  return signature ? utils.bytes.bs58.encode(signature) : "unsigned-tx";
};

const metaFrom = (meta: TransactionMetadata): LiteSVMTransactionMeta => ({
  logMessages: meta.logs(),
  computeUnitsConsumed: meta.computeUnitsConsumed(),
});

const failedFrom = (
  failed: FailedTransactionMetadata,
): LiteSVMTransactionResultWithMeta => ({
  result: failed.err().toString(),
  meta: metaFrom(failed.meta()),
});

const throwFailedTransaction = (
  failed: FailedTransactionMetadata,
  tx: Transaction | VersionedTransaction,
): never => {
  const transactionMessage = [
    failed.err().toString(),
    failed.toString(),
  ].join("\n");
  throw new SendTransactionError({
    action: "send",
    signature: txSignature(tx),
    transactionMessage,
    logs: failed.meta().logs(),
  });
};

export class BanksClient {
  constructor(private readonly svm: LiteSVM) {}

  async getAccount(publicKey: PublicKey): Promise<AccountInfo<Buffer> | null> {
    return asBufferAccount(this.svm.getAccount(publicKey));
  }

  async getBalance(publicKey: PublicKey): Promise<bigint> {
    return this.svm.getBalance(publicKey) ?? 0n;
  }

  async getRent() {
    return this.svm.getRent();
  }

  async getClock(): Promise<Clock> {
    return this.svm.getClock();
  }

  async getSlot(): Promise<bigint> {
    return this.svm.getClock().slot;
  }

  async getLatestBlockhash(): Promise<[string, bigint]> {
    return [this.svm.latestBlockhash(), 0n];
  }

  async processTransaction(
    tx: Transaction | VersionedTransaction,
  ): Promise<LiteSVMTransactionMeta> {
    const result = this.svm.sendTransaction(tx);
    if (result instanceof FailedTransactionMetadata) {
      throwFailedTransaction(result, tx);
    }
    return metaFrom(result);
  }

  async tryProcessTransaction(
    tx: Transaction | VersionedTransaction,
  ): Promise<LiteSVMTransactionResultWithMeta> {
    const result = this.svm.sendTransaction(tx);
    if (result instanceof FailedTransactionMetadata) {
      return failedFrom(result);
    }
    return {
      result: null,
      meta: metaFrom(result),
    };
  }

  async simulateTransaction(
    tx: Transaction | VersionedTransaction,
  ): Promise<LiteSVMTransactionMeta | LiteSVMTransactionResultWithMeta> {
    const result = this.svm.simulateTransaction(tx);
    if (result instanceof FailedTransactionMetadata) {
      return failedFrom(result);
    }
    return metaFrom(result.meta());
  }
}

export class ProgramTestContext {
  readonly banksClient: BanksClient;

  constructor(
    readonly svm: LiteSVM,
    readonly payer: Keypair,
  ) {
    this.banksClient = new BanksClient(svm);
  }

  setAccount(publicKey: PublicKey, account: AccountInfo<Buffer>): void {
    this.svm.setAccount(publicKey, normalizeAccount(account));
  }

  setClock(clock: Clock): void {
    this.svm.setClock(clock);
    syncRecentSlotHash(this.svm);
  }

  warpToSlot(slot: bigint): void {
    this.svm.warpToSlot(slot);
    syncRecentSlotHash(this.svm);
  }

  warpToEpoch(epoch: bigint): void {
    const schedule = this.svm.getEpochSchedule();
    this.warpToSlot(epoch * schedule.slotsPerEpoch);
  }
}

export class BankrunProvider extends AnchorLiteSVMProvider {
  readonly context: ProgramTestContext;

  constructor(context: ProgramTestContext) {
    super(context.svm as never, new Wallet(context.payer));
    this.context = context;
  }
}

const programPath = (workspacePath: string, name: string): string => {
  const normalizedName = name.replace(/-/g, "_");
  const candidates = [
    path.join(workspacePath, "tests", "fixtures", `${normalizedName}.so`),
    path.join(workspacePath, "target", "deploy", `${normalizedName}.so`),
  ];
  const found = candidates.find((candidate) => fs.existsSync(candidate));
  if (!found) {
    throw new Error(
      `Could not find LiteSVM program artifact for ${name}; tried ${candidates.join(
        ", ",
      )}`,
    );
  }
  return found;
};

const dedupePrograms = (programs: AddedProgram[]): AddedProgram[] => {
  const byProgramId = new Map<string, AddedProgram>();
  for (const program of programs) {
    byProgramId.set(program.programId.toBase58(), program);
  }
  return [...byProgramId.values()];
};

export async function startAnchor(
  workspacePath: string,
  programs: AddedProgram[],
  accounts: AddedAccount[],
): Promise<ProgramTestContext> {
  const svm = new LiteSVM()
    .withLamports(PAYER_LAMPORTS * 2n)
    .withTransactionHistory(0n)
    .withLogBytesLimit(undefined);
  svm.warpToSlot(INITIAL_SLOT);
  const clock = svm.getClock();
  svm.setClock(
    new Clock(
      clock.slot,
      clock.epochStartTimestamp,
      clock.epoch,
      clock.leaderScheduleEpoch,
      INITIAL_UNIX_TIMESTAMP,
    ),
  );
  syncRecentSlotHash(svm);
  const payer = Keypair.generate();
  const airdropResult = svm.airdrop(payer.publicKey, PAYER_LAMPORTS);
  if (airdropResult instanceof FailedTransactionMetadata) {
    throw new Error(`LiteSVM payer airdrop failed: ${airdropResult.toString()}`);
  }

  for (const program of dedupePrograms([...LOCAL_PROGRAMS, ...programs])) {
    svm.addProgramFromFile(
      program.programId,
      programPath(workspacePath, program.name),
    );
  }

  for (const account of accounts) {
    svm.setAccount(account.address, normalizeAccount(account.info));
  }

  return new ProgramTestContext(svm, payer);
}
