/**
 * Mainnet-only fee simulation (simulateTransaction). NEVER submits/broadcasts.
 * Field used: simulation.minResourceFee (stroops) + inclusion fee 100 stroops.
 */
import pkg from "@stellar/stellar-sdk";
const { Networks, rpc, TransactionBuilder, Operation, xdr, Address, Account, nativeToScVal, Contract } = pkg;
import fs from "fs";
import path from "path";
import crypto from "crypto";

const RPC_URL = process.env.STELLAR_RPC_URL || "https://mainnet.sorobanrpc.com";
const PASS = Networks.PUBLIC;
const SRC = process.env.SOURCE_G || "GA5ZSEJYB37JRC5AVCIA5MOP4RHTM335X2KGX3IHOJAPP5RE34K4KZVN";
const WASM_DIR =
  process.env.WASM_DIR ||
  "D:/Git/orgs/0xVeraHQ/projects/artifex/artifex-contracts/target/wasm32v1-none/release";
const OUT =
  process.env.OUT ||
  "D:/Git/orgs/0xVeraHQ/projects/artifex/artifex-contracts-wt-mainnet-d576/scripts/mainnet-sim-fees.json";

const CONTRACTS = [
  "artifex_roles",
  "artifex_fee_collector",
  "artifex_connoisseur",
  "artifex_cert_registry",
  "artifex_whole_nft",
  "artifex_participation",
  "artifex_marketplace",
  "artifex_distribution",
];

const INCLUSION_STROOPS = 100n;

function stroopsToXlm(s) {
  const n = BigInt(s);
  const whole = n / 10_000_000n;
  const frac = (n % 10_000_000n).toString().padStart(7, "0");
  return `${whole}.${frac}`;
}

function totalFeeXlm(minResourceFee) {
  return stroopsToXlm(BigInt(minResourceFee) + INCLUSION_STROOPS);
}

async function simulate(server, account, op) {
  const tx = new TransactionBuilder(account, {
    fee: String(INCLUSION_STROOPS),
    networkPassphrase: PASS,
  })
    .addOperation(op)
    .setTimeout(180)
    .build();
  const sim = await server.simulateTransaction(tx);
  if (rpc.Api.isSimulationError(sim)) {
    return { ok: false, error: String(sim.error || "simulation_error") };
  }
  return {
    ok: true,
    minResourceFee: String(sim.minResourceFee),
    feeXlm: totalFeeXlm(sim.minResourceFee),
    latestLedger: sim.latestLedger,
  };
}

async function main() {
  const server = new rpc.Server(RPC_URL, { allowHttp: false });
  const health = await server.getHealth();
  const accEntry = await server.getAccount(SRC);
  const timestamp = new Date().toISOString();
  const baseSeq = BigInt(accEntry.sequenceNumber());

  const rows = [];
  const blockers = [];

  // --- Uploads (real Artifex WASM) ---
  for (let i = 0; i < CONTRACTS.length; i++) {
    const name = CONTRACTS[i];
    const wasmPath = path.join(WASM_DIR, `${name}.wasm`);
    const wasm = fs.readFileSync(wasmPath);
    const wasmHash = crypto.createHash("sha256").update(wasm).digest("hex");
    const account = new Account(SRC, (baseSeq + BigInt(i)).toString());
    const op = Operation.invokeHostFunction({
      func: xdr.HostFunction.hostFunctionTypeUploadContractWasm(wasm),
      auth: [],
    });
    const sim = await simulate(server, account, op);
    rows.push({
      operation: "upload",
      contract: name,
      wasmBytes: wasm.length,
      wasmHash,
      ...sim,
    });
    console.log(
      `upload ${name}: ${sim.ok ? sim.feeXlm + " XLM" : "FAIL " + sim.error.slice(0, 120)}`,
    );
  }

  // --- Create/instance against Artifex wasm hash (expected fail if not on ledger) ---
  for (let i = 0; i < CONTRACTS.length; i++) {
    const name = CONTRACTS[i];
    const wasmPath = path.join(WASM_DIR, `${name}.wasm`);
    const wasm = fs.readFileSync(wasmPath);
    const wasmHash = crypto.createHash("sha256").update(wasm).digest();
    const account = new Account(SRC, (baseSeq + 100n + BigInt(i)).toString());
    const salt = crypto.createHash("sha256").update(`artifex-sim-create-${name}`).digest();
    const op = Operation.createCustomContract({
      address: Address.fromString(SRC),
      wasmHash,
      salt,
    });
    const sim = await simulate(server, account, op);
    rows.push({
      operation: "create",
      contract: name,
      wasmHash: wasmHash.toString("hex"),
      ...sim,
    });
    if (!sim.ok) {
      blockers.push({
        operation: "create",
        contract: name,
        error: sim.error.split("\n")[0],
      });
    }
    console.log(
      `create ${name}: ${sim.ok ? sim.feeXlm + " XLM" : "FAIL " + sim.error.split("\\n")[0].slice(0, 120)}`,
    );
  }

  // --- Init (needs instance) — try deterministic address from create; expect fail ---
  const platform = Address.fromString(SRC);
  const initSpecs = {
    artifex_roles: () => [platform.toScVal()],
    artifex_connoisseur: () => [platform.toScVal()],
    artifex_fee_collector: () => [
      platform.toScVal(),
      Address.fromString("CCW67TSZV3SSS2HXMBQ5JFGCKJNXKZM7UQUWUZPUTHXSTZLEO7SJMI75").toScVal(),
      nativeToScVal(5_000_000, { type: "i128" }),
      nativeToScVal(1_000_000, { type: "i128" }),
    ],
    artifex_cert_registry: () => [
      platform.toScVal(),
      platform.toScVal(),
      // placeholders — will fail before arg validation if contract missing
      platform.toScVal(),
      platform.toScVal(),
    ],
    artifex_whole_nft: () => [platform.toScVal()],
    artifex_participation: () => [platform.toScVal()],
    artifex_marketplace: () => [platform.toScVal(), platform.toScVal()],
    artifex_distribution: () => [platform.toScVal(), platform.toScVal()],
  };

  for (let i = 0; i < CONTRACTS.length; i++) {
    const name = CONTRACTS[i];
    // Deterministic C… from source+salt (same salt as create attempt)
    const wasmPath = path.join(WASM_DIR, `${name}.wasm`);
    const wasm = fs.readFileSync(wasmPath);
    const wasmHash = crypto.createHash("sha256").update(wasm).digest();
    const salt = crypto.createHash("sha256").update(`artifex-sim-create-${name}`).digest();
    const contractId = Address.fromString(SRC).toScAddress();
    // Use SDK helper if available; else placeholder impossible ID
    let contractAddress;
    try {
      // Precompute contract ID the network would assign
      const preimage = xdr.HashIdPreimage.envelopeTypeContractId(
        new xdr.HashIdPreimageContractId({
          networkId: crypto.createHash("sha256").update(PASS).digest(),
          contractIdPreimage: xdr.ContractIdPreimage.contractIdPreimageFromAddress(
            new xdr.ContractIdPreimageFromAddress({
              address: Address.fromString(SRC).toScAddress(),
              salt,
            }),
          ),
        }),
      );
      const idHash = crypto.createHash("sha256").update(preimage.toXDR()).digest();
      // StrKey encode contract
      const { StrKey } = pkg;
      contractAddress = StrKey.encodeContract(idHash);
    } catch (e) {
      contractAddress = null;
      blockers.push({ operation: "init", contract: name, error: `id_precompute:${e.message}` });
    }

    if (contractAddress) {
      const account = new Account(SRC, (baseSeq + 200n + BigInt(i)).toString());
      const c = new Contract(contractAddress);
      const args = initSpecs[name] ? initSpecs[name]() : [platform.toScVal()];
      const op = c.call("init", ...args);
      const sim = await simulate(server, account, op);
      rows.push({
        operation: "init",
        contract: name,
        contractId: contractAddress,
        ...sim,
      });
      if (!sim.ok) {
        blockers.push({
          operation: "init",
          contract: name,
          error: sim.error.split("\n")[0],
        });
      }
      console.log(
        `init ${name}: ${sim.ok ? sim.feeXlm + " XLM" : "FAIL " + sim.error.split("\\n")[0].slice(0, 120)}`,
      );
    }
  }

  // --- Typical invokes (seed/QA) — expect fail without instances ---
  const invokeFlows = [
    { flow: "propose_cert", contract: "artifex_cert_registry", fn: "propose_cert" },
    { flow: "create_offer", contract: "artifex_participation", fn: "create_offer" },
    { flow: "buy_primary", contract: "artifex_participation", fn: "buy_primary" },
    { flow: "list_secondary", contract: "artifex_marketplace", fn: "list" },
    { flow: "list_whole", contract: "artifex_whole_nft", fn: "list_for_sale" },
    { flow: "buy_whole", contract: "artifex_whole_nft", fn: "buy_sale" },
    { flow: "distribute", contract: "artifex_distribution", fn: "distribute" },
  ];

  for (let i = 0; i < invokeFlows.length; i++) {
    const { flow, contract: cname, fn } = invokeFlows[i];
    const row = rows.find((r) => r.operation === "init" && r.contract === cname);
    const contractId = row?.contractId;
    if (!contractId) {
      blockers.push({ operation: flow, contract: cname, error: "no_contract_id" });
      rows.push({ operation: flow, contract: cname, ok: false, error: "no_contract_id" });
      continue;
    }
    const account = new Account(SRC, (baseSeq + 300n + BigInt(i)).toString());
    const c = new Contract(contractId);
    // Minimal placeholder args — simulation fails on missing instance before deep arg checks
    const op = c.call(fn, platform.toScVal());
    const sim = await simulate(server, account, op);
    rows.push({ operation: flow, contract: cname, fn, contractId, ...sim });
    if (!sim.ok) {
      blockers.push({ operation: flow, contract: cname, error: sim.error.split("\n")[0] });
    }
    console.log(
      `invoke ${flow}: ${sim.ok ? sim.feeXlm + " XLM" : "FAIL " + sim.error.split("\\n")[0].slice(0, 120)}`,
    );
  }

  const uploadOk = rows.filter((r) => r.operation === "upload" && r.ok);
  const uploadSumStroops = uploadOk.reduce(
    (a, r) => a + BigInt(r.minResourceFee) + INCLUSION_STROOPS,
    0n,
  );

  const report = {
    meta: {
      rpcUrl: RPC_URL,
      sourceAccount: SRC,
      networkPassphrase: PASS,
      timestamp,
      health,
      feeField:
        "minResourceFee (stroops) from rpc.Server.simulateTransaction; reported XLM = (minResourceFee + inclusion 100 stroops) / 1e7. Nothing submitted/broadcast.",
      wasmDir: WASM_DIR,
    },
    rows,
    blockers,
    totals: {
      uploadRawXlm: stroopsToXlm(uploadSumStroops),
      uploadPlus30Xlm: stroopsToXlm((uploadSumStroops * 130n) / 100n),
      createInitInvoke: "BLOCKED — see blockers",
    },
  };

  fs.writeFileSync(OUT, JSON.stringify(report, null, 2));
  console.log("\nWrote", OUT);
  console.log("Upload total XLM", report.totals.uploadRawXlm, "+30%", report.totals.uploadPlus30Xlm);
  console.log("Blockers", blockers.length);
}

main().catch((e) => {
  console.error(e);
  process.exit(1);
});
