import { Connection, PublicKey } from "@solana/web3.js";
import { Program, AnchorProvider } from "@coral-xyz/anchor";
import { readFileSync, writeFileSync } from "fs";
import { resolve } from "path";
import { loadAltStagingEnv } from "./lib/env";
import { MARGINFI_IDL, MarginfiIdlType } from "@mrgnlabs/marginfi-client-v2";

/**
 * Fetch oracle addresses from existing marginfi mainnet banks.
 *
 * Uses the marginfi IDL to properly deserialize Bank accounts and extract
 * oracle configuration.
 *
 * Usage:
 *   npx ts-node --transpile-only ops/alt-staging/scripts/fetch-marginfi-oracles.ts
 *   npx ts-node --transpile-only ops/alt-staging/scripts/fetch-marginfi-oracles.ts --all-banks
 *
 * Options:
 *   --all-banks  Query ALL banks on-chain, not just those in the metadata cache
 */

const MARGINFI_PROGRAM_ID = "MFv2hWf31Z9kbCa1snEPYctwafyhdvnV7FZnsebVacA";
const MARGINFI_GROUP = "4qp6Fx6tnZkY5Wropq9wUYgtFxXKwE6viZxFHg3rdAG8";

// Oracle setup types from OracleSetup enum
const ORACLE_SETUP_MAP: Record<string, number> = {
  none: 0,
  pythLegacy: 1,
  switchboardV2: 2,
  pythPushOracle: 3,
  switchboardPull: 4,
  stakedWithPythPush: 5,
  kaminoPythPush: 6,
  kaminoSwitchboardPull: 7,
  fixed: 8,
  driftPythPull: 9,
  driftSwitchboardPull: 10,
  solendPythPull: 11,
  solendSwitchboardPull: 12,
  juplendPythPull: 14,
  juplendSwitchboardPull: 15,
};

// Map from regular oracle setup to JupLend equivalent
function mapToJuplendOracleSetup(setupName: string): { name: string; value: number } {
  const lower = setupName.toLowerCase();
  if (lower.includes("pyth")) {
    return { name: "juplendPythPull", value: 14 };
  }
  if (lower.includes("switchboard")) {
    return { name: "juplendSwitchboardPull", value: 15 };
  }
  // Default to Pyth
  console.warn(`Unknown oracle setup: ${setupName}, defaulting to juplendPythPull`);
  return { name: "juplendPythPull", value: 14 };
}

type AssetConfig = {
  symbol: string;
  lending: string;
  fTokenMint: string;
  mint: string;
};

type AssetsFile = {
  juplendProgram: string;
  lendingAdmin: string;
  bankSeed: number;
  assets: AssetConfig[];
};

type BankOracleInfo = {
  symbol: string;
  mint: string;
  bankAddress: string;
  group: string;
  oracleSetup: string;
  oracleSetupValue: number;
  oracle: string | null;
  allOracles: string[];
};

type OracleResult = {
  symbol: string;
  mint: string;
  oracle: string;
  oracleSetup: string;
  oracleSetupValue: number;
  juplendOracleSetup: string;
  juplendOracleSetupValue: number;
  totalBanksFound: number;
  hasMultipleOracleTypes: boolean;
  selectedFrom: string; // which group/bank was selected
};

async function main() {
  loadAltStagingEnv();

  const useAllBanks = process.argv.includes("--all-banks");

  const rpcUrl =
    process.env.MAINNET_RPC_URL ||
    process.env.ALT_STAGING_RPC_URL ||
    "https://api.mainnet-beta.solana.com";

  console.log("=== fetch-marginfi-oracles ===");
  console.log("RPC:", rpcUrl);
  console.log("Marginfi Program:", MARGINFI_PROGRAM_ID);
  console.log("Marginfi Main Group:", MARGINFI_GROUP);
  console.log("Mode:", useAllBanks ? "ALL banks on-chain" : "metadata cache only");

  const connection = new Connection(rpcUrl, "confirmed");

  // Setup marginfi program with IDL
  const idl = { ...MARGINFI_IDL, address: MARGINFI_PROGRAM_ID };
  const provider = new AnchorProvider(
    connection,
    {} as any, // Read-only, no wallet needed
    { commitment: "confirmed" }
  );
  const program = new Program<MarginfiIdlType>(idl as any, provider);

  // Load asset config
  const assetsPath = resolve(__dirname, "..", "configs", "juplend-assets.json");
  const assetsFile: AssetsFile = JSON.parse(readFileSync(assetsPath, "utf-8"));

  // Build mint list from config
  const mintSymbolMap = new Map<string, string>();
  for (const asset of assetsFile.assets) {
    if (asset.mint) {
      mintSymbolMap.set(asset.mint, asset.symbol);
    }
  }

  console.log("\nLooking for banks with these mints:");
  for (const [mint, symbol] of mintSymbolMap) {
    console.log(`  ${symbol}: ${mint}`);
  }

  let bankPubkeys: PublicKey[];
  let allBanks: any[];

  if (useAllBanks) {
    // Query banks by mint using memcmp filters (more targeted than fetching all)
    console.log("\nFetching banks by mint from on-chain...");

    const allRelevantBanks: { publicKey: PublicKey; account: any }[] = [];

    // Bank account layout: 8 bytes discriminator, then mint (32 bytes)
    const MINT_OFFSET = 8;

    for (const [mint, symbol] of mintSymbolMap) {
      console.log(`  Querying banks for ${symbol}...`);
      try {
        const banksForMint = await program.account.bank.all([
          {
            memcmp: {
              offset: MINT_OFFSET,
              bytes: mint,
            },
          },
        ]);
        console.log(`    Found ${banksForMint.length} bank(s) for ${symbol}`);
        allRelevantBanks.push(...banksForMint);
      } catch (e: any) {
        // Try without filter if memcmp fails
        console.log(`    Filter failed for ${symbol}, trying getProgramAccounts directly...`);
        try {
          const accounts = await connection.getProgramAccounts(
            new PublicKey(MARGINFI_PROGRAM_ID),
            {
              filters: [
                { memcmp: { offset: MINT_OFFSET, bytes: mint } },
              ],
            }
          );
          console.log(`    Found ${accounts.length} raw account(s) for ${symbol}`);
          for (const { pubkey, account } of accounts) {
            try {
              const decoded = program.coder.accounts.decode("bank", account.data);
              allRelevantBanks.push({ publicKey: pubkey, account: decoded });
            } catch (decodeErr) {
              console.log(`    Could not decode bank ${pubkey.toBase58()}`);
            }
          }
        } catch (e2) {
          console.log(`    Could not query banks for ${symbol}: ${e2}`);
        }
      }
    }

    console.log(`\nFound ${allRelevantBanks.length} total banks matching our mints`);

    bankPubkeys = allRelevantBanks.map((acc) => acc.publicKey);
    allBanks = allRelevantBanks.map((acc) => acc.account);
  } else {
    // Use metadata cache (faster, default)
    console.log("\nFetching bank metadata from marginfi cache...");
    const bankMetadataUrl =
      "https://storage.googleapis.com/mrgn-public/mrgn-bank-metadata-cache.json";
    const bankMetadataResponse = await fetch(bankMetadataUrl);
    const bankMetadata: any[] = await bankMetadataResponse.json();
    console.log(`Found ${bankMetadata.length} banks in metadata cache`);

    // Filter to banks we care about
    const relevantBanks = bankMetadata.filter((meta) =>
      mintSymbolMap.has(meta.tokenAddress)
    );
    console.log(`Found ${relevantBanks.length} banks matching our mints`);

    // Fetch all relevant banks
    bankPubkeys = relevantBanks.map((meta) => new PublicKey(meta.bankAddress));
    allBanks = await program.account.bank.fetchMultiple(bankPubkeys);
  }

  // Group results by mint
  const mintBanksMap = new Map<string, BankOracleInfo[]>();

  for (let i = 0; i < allBanks.length; i++) {
    const bank = allBanks[i];
    const bankAddress = bankPubkeys[i];

    if (!bank) {
      console.log(`Skipping null bank: ${bankAddress.toBase58()}`);
      continue;
    }

    const mintStr = bank.mint.toString();
    const symbol = mintSymbolMap.get(mintStr);
    if (!symbol) continue;

    // Get oracle setup type
    let oracleSetup = "unknown";
    let oracleSetupValue = 0;
    if (bank.config.oracleSetup) {
      const setupKeys = Object.keys(bank.config.oracleSetup);
      if (setupKeys.length > 0) {
        oracleSetup = setupKeys[0];
        oracleSetupValue = ORACLE_SETUP_MAP[oracleSetup] ?? 0;
      }
    }

    // Get all oracle keys (filter out default pubkey)
    const allOracles = bank.config.oracleKeys
      .filter((key: PublicKey) => key.toString() !== PublicKey.default.toString())
      .map((key: PublicKey) => key.toString());

    const oracle = allOracles.length > 0 ? allOracles[0] : null;

    const info: BankOracleInfo = {
      symbol,
      mint: mintStr,
      bankAddress: bankAddress.toString(),
      group: bank.group.toString(),
      oracleSetup,
      oracleSetupValue,
      oracle,
      allOracles,
    };

    if (!mintBanksMap.has(mintStr)) {
      mintBanksMap.set(mintStr, []);
    }
    mintBanksMap.get(mintStr)!.push(info);

    console.log(`\nFound ${symbol} bank: ${bankAddress.toBase58()}`);
    console.log(`  Group: ${bank.group.toString()}`);
    console.log(`  Oracle Setup: ${oracleSetup} (${oracleSetupValue})`);
    console.log(`  Oracle: ${oracle || "N/A"}`);
    if (allOracles.length > 1) {
      console.log(`  All Oracles: ${allOracles.join(", ")}`);
    }
  }

  // Build final results - prefer Switchboard, then Pyth
  const results: OracleResult[] = [];

  for (const [mint, banks] of mintBanksMap) {
    if (banks.length === 0) continue;

    // Group ALL banks by oracle type (not just main group - Switchboard might be in other groups)
    const oracleGroups = new Map<string, BankOracleInfo[]>();
    for (const bank of banks) {
      if (!bank.oracle) continue;
      const key = `${bank.oracle}:${bank.oracleSetup}`;
      if (!oracleGroups.has(key)) {
        oracleGroups.set(key, []);
      }
      oracleGroups.get(key)!.push(bank);
    }

    // Find oracle config - prefer Switchboard, then Pyth, then others
    // Within same type, prefer main group
    let selected: BankOracleInfo | null = null;
    let selectedPriority = -1; // 2 = switchboard, 1 = pyth, 0 = other

    for (const [, groupBanks] of oracleGroups) {
      const setup = groupBanks[0].oracleSetup.toLowerCase();
      const isSwitchboard = setup.includes("switchboard");
      const isPyth = setup.includes("pyth");
      const priority = isSwitchboard ? 2 : isPyth ? 1 : 0;

      // Check if any bank in this group is from the main marginfi group
      const mainGroupBank = groupBanks.find((b) => b.group === MARGINFI_GROUP);
      const bankToUse = mainGroupBank || groupBanks[0];

      if (priority > selectedPriority) {
        selectedPriority = priority;
        selected = bankToUse;
      } else if (priority === selectedPriority && !selected) {
        selected = bankToUse;
      }
    }

    if (!selected || !selected.oracle) {
      console.warn(`No valid oracle found for ${banks[0].symbol}`);
      continue;
    }

    const juplendSetup = mapToJuplendOracleSetup(selected.oracleSetup);
    const hasMultipleTypes = oracleGroups.size > 1;

    if (hasMultipleTypes) {
      console.log(`\n⚠️  Multiple oracle types for ${selected.symbol}:`);
      for (const [key, groupBanks] of oracleGroups) {
        console.log(`    - ${groupBanks[0].oracleSetup}: ${groupBanks.length} bank(s)`);
      }
    }

    results.push({
      symbol: selected.symbol,
      mint,
      oracle: selected.oracle,
      oracleSetup: selected.oracleSetup,
      oracleSetupValue: selected.oracleSetupValue,
      juplendOracleSetup: juplendSetup.name,
      juplendOracleSetupValue: juplendSetup.value,
      totalBanksFound: banks.length,
      hasMultipleOracleTypes: hasMultipleTypes,
      selectedFrom: selected.group === MARGINFI_GROUP ? "mainGroup" : selected.group,
    });
  }

  // Output results
  const outputPath = resolve(__dirname, "..", "configs", "oracles.json");
  const output = {
    fetchedAt: new Date().toISOString(),
    marginfiProgram: MARGINFI_PROGRAM_ID,
    marginfiMainGroup: MARGINFI_GROUP,
    oracles: results,
    // Include full bank details for reference
    details: Object.fromEntries(
      Array.from(mintBanksMap.entries()).map(([mint, banks]) => [
        mint,
        { symbol: banks[0]?.symbol, banks },
      ])
    ),
  };

  writeFileSync(outputPath, JSON.stringify(output, null, 2));
  console.log(`\nSaved ${results.length} oracle configs to: ${outputPath}`);

  // Print summary
  console.log("\n=== Oracle Summary ===");
  for (const r of results) {
    const warning = r.hasMultipleOracleTypes ? " ⚠️" : "";
    const source = r.selectedFrom === "mainGroup" ? "" : ` (from ${r.selectedFrom.slice(0, 8)}...)`;
    console.log(
      `${r.symbol}: ${r.oracle} (${r.oracleSetup} → ${r.juplendOracleSetup}) [${r.totalBanksFound} banks]${warning}${source}`
    );
  }

  // Check for missing mints
  const foundMints = new Set(results.map((r) => r.mint));
  const missingAssets = assetsFile.assets.filter((a) => !foundMints.has(a.mint));

  if (missingAssets.length > 0) {
    console.log("\n=== Missing Oracles ===");
    console.log("These assets were not found in marginfi mainnet banks:");
    for (const asset of missingAssets) {
      console.log(`  - ${asset.symbol} (${asset.mint})`);
    }
    console.log("\nYou'll need to manually provide oracle addresses for these assets.");
  }
}

main().catch((err) => {
  console.error(err);
  process.exit(1);
});
