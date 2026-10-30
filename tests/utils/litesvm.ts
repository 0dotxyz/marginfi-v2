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
import { address, getTransactionDecoder, lamports } from "@solana/kit";
import {
  Clock,
  FailedTransactionMetadata,
  LiteSVM as KitLiteSVM,
  TransactionMetadata,
} from "litesvm";
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

// The Anchor provider uses web3.js objects, while recent LiteSVM releases use
// Solana Kit. Keep that conversion at the test-runtime boundary.
class LiteSVM {
  private readonly inner = new KitLiteSVM();

  withLamports(amount: bigint): this {
    this.inner.withLamports(amount);
    return this;
  }

  withTransactionHistory(capacity: bigint): this {
    this.inner.withTransactionHistory(capacity);
    return this;
  }

  withLogBytesLimit(limit?: bigint): this {
    this.inner.withLogBytesLimit(limit);
    return this;
  }

  getAccount(key: PublicKey): AccountInfo<Uint8Array> | null {
    const account = this.inner.getAccount(address(key.toBase58()));
    if (!account.exists) return null;
    return {
      data: account.data,
      executable: account.executable,
      lamports: Number(account.lamports),
      owner: new PublicKey(account.programAddress),
      rentEpoch: 0,
    };
  }

  setAccount(key: PublicKey, account: AccountInfo<Uint8Array>): void {
    this.inner.setAccount({
      address: address(key.toBase58()),
      data: account.data,
      executable: account.executable,
      lamports: lamports(BigInt(account.lamports)),
      programAddress: address(account.owner.toBase58()),
      space: BigInt(account.data.length),
    });
  }

  getBalance(key: PublicKey) {
    return this.inner.getBalance(address(key.toBase58()));
  }
  getRent() {
    return this.inner.getRent();
  }
  getClock() {
    return this.inner.getClock();
  }
  setClock(clock: Clock) {
    this.inner.setClock(clock);
  }
  getEpochSchedule() {
    return this.inner.getEpochSchedule();
  }
  getSlotHashes() {
    return this.inner.getSlotHashes();
  }
  setSlotHashes(hashes: ReturnType<KitLiteSVM["getSlotHashes"]>) {
    this.inner.setSlotHashes(hashes);
  }
  latestBlockhash() {
    return this.inner.latestBlockhash();
  }
  warpToSlot(slot: bigint) {
    this.inner.warpToSlot(slot);
  }
  airdrop(key: PublicKey, amount: bigint) {
    return this.inner.airdrop(address(key.toBase58()), lamports(amount));
  }
  addProgramFromFile(key: PublicKey, file: string) {
    this.inner.addProgramFromFile(address(key.toBase58()), file);
  }

  private toKitTransaction(tx: Transaction | VersionedTransaction) {
    const bytes =
      tx instanceof Transaction
        ? tx.serialize({ requireAllSignatures: false, verifySignatures: false })
        : tx.serialize();
    return getTransactionDecoder().decode(bytes);
  }

  sendTransaction(tx: Transaction | VersionedTransaction) {
    return this.inner.sendTransaction(this.toKitTransaction(tx));
  }

  simulateTransaction(tx: Transaction | VersionedTransaction) {
    return this.inner.simulateTransaction(this.toKitTransaction(tx));
  }
}

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
  account: AccountInfo<Uint8Array> | null
): AccountInfo<Buffer> | null => {
  if (!account) return null;
  return {
    ...account,
    data: Buffer.from(account.data),
  };
};

const normalizeAccount = (
  account: AccountInfo<Buffer>
): AccountInfo<Uint8Array> => ({
  ...account,
  data: new Uint8Array(account.data),
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
  failed: FailedTransactionMetadata
): LiteSVMTransactionResultWithMeta => ({
  result: failed.err().toString(),
  meta: metaFrom(failed.meta()),
});

const throwFailedTransaction = (
  failed: FailedTransactionMetadata,
  tx: Transaction | VersionedTransaction
): never => {
  const transactionMessage = [failed.err().toString(), failed.toString()].join(
    "\n"
  );
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
    tx: Transaction | VersionedTransaction
  ): Promise<LiteSVMTransactionMeta> {
    const result = this.svm.sendTransaction(tx);
    if (result instanceof FailedTransactionMetadata) {
      return throwFailedTransaction(result, tx);
    }
    return metaFrom(result);
  }

  async tryProcessTransaction(
    tx: Transaction | VersionedTransaction
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
    tx: Transaction | VersionedTransaction
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

  constructor(readonly svm: LiteSVM, readonly payer: Keypair) {
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
    // The provider checks against its own older LiteSVM's error class, so
    // translate failures before they reach it.
    const client = Object.create(context.svm) as LiteSVM;
    client.sendTransaction = (tx) => {
      const result = context.svm.sendTransaction(tx);
      if (result instanceof FailedTransactionMetadata)
        throwFailedTransaction(result, tx);
      return result;
    };
    client.simulateTransaction = (tx) => {
      const result = context.svm.simulateTransaction(tx);
      if (result instanceof FailedTransactionMetadata)
        throwFailedTransaction(result, tx);
      return result;
    };
    super(client as never, new Wallet(context.payer));
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
        ", "
      )}`
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
  accounts: AddedAccount[]
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
      INITIAL_UNIX_TIMESTAMP
    )
  );
  syncRecentSlotHash(svm);
  const payer = Keypair.generate();
  const airdropResult = svm.airdrop(payer.publicKey, PAYER_LAMPORTS);
  if (airdropResult instanceof FailedTransactionMetadata) {
    throw new Error(
      `LiteSVM payer airdrop failed: ${airdropResult.toString()}`
    );
  }

  for (const program of dedupePrograms([...LOCAL_PROGRAMS, ...programs])) {
    svm.addProgramFromFile(
      program.programId,
      programPath(workspacePath, program.name)
    );
  }

  for (const account of accounts) {
    svm.setAccount(account.address, normalizeAccount(account.info));
  }

  return new ProgramTestContext(svm, payer);
}
