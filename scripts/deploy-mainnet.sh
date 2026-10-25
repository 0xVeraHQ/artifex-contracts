#!/usr/bin/env bash
# Deploy Artifex Soroban contracts to Stellar MAINNET.
# HARD STOP: refuses unless ARTIFEX_CONFIRM_MAINNET=JOEL_CONFIRMED.
# Does NOT Friendbot-fund. See deploy-mainnet.ps1 for the Windows twin.
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT"

if [[ "${ARTIFEX_CONFIRM_MAINNET:-}" != "JOEL_CONFIRMED" ]]; then
  echo "HARD STOP: set ARTIFEX_CONFIRM_MAINNET=JOEL_CONFIRMED after Joel explicitly confirms. No mainnet deploy." >&2
  exit 2
fi

IDENTITY="${STELLAR_DEPLOYER_IDENTITY:-artifex-mn-deployer}"
if [[ -n "${STELLAR_DEPLOYER_SECRET:-}" ]]; then
  printf '%s\n' "$STELLAR_DEPLOYER_SECRET" | stellar keys add "$IDENTITY" --secret-key >/dev/null 2>&1 || true
fi
if ! stellar keys address "$IDENTITY" >/dev/null 2>&1; then
  echo "Missing identity '$IDENTITY'. Import STELLAR_DEPLOYER_SECRET or generate + fund on mainnet." >&2
  exit 1
fi

ADDR="$(stellar keys address "$IDENTITY")"
echo "Deployer (mainnet): $ADDR"
echo "NO Friendbot — account must already be funded."

stellar contract build

WASM_DIR="$ROOT/target/wasm32v1-none/release"
OUT_DIR="$ROOT/deployments"
mkdir -p "$OUT_DIR"

deploy_wasm() {
  local name="$1"
  local wasm="$WASM_DIR/${name}.wasm"
  [[ -f "$wasm" ]] || { echo "missing $wasm" >&2; exit 1; }
  local id
  id="$(stellar contract deploy --wasm "$wasm" --source "$IDENTITY" --network mainnet --alias "$name" | tail -n1 | tr -d '\r')"
  echo "Deployed $name -> $id"
  printf '%s' "$id"
}

roles="$(deploy_wasm artifex_roles)"
fee="$(deploy_wasm artifex_fee_collector)"
cnn="$(deploy_wasm artifex_connoisseur)"
cert="$(deploy_wasm artifex_cert_registry)"
nft="$(deploy_wasm artifex_whole_nft)"
part="$(deploy_wasm artifex_participation)"
mkt="$(deploy_wasm artifex_marketplace)"
dist="$(deploy_wasm artifex_distribution)"

USDC_ISSUER="GA5ZSEJYB37JRC5AVCIA5MOP4RHTM335X2KGX3IHOJAPP5RE34K4KZVN"
usdc_sac="$(stellar contract id asset --asset "USDC:$USDC_ISSUER" --network mainnet | tail -n1 | tr -d '\r')"
xlm_sac="$(stellar contract id asset --asset native --network mainnet | tail -n1 | tr -d '\r')"
echo "USDC SAC (mainnet): $usdc_sac"
echo "XLM SAC (mainnet): $xlm_sac"

# Mainnet symbolic fees (Joel): 0.5 XLM / 0.1 USDC (not Testnet 5 XLM / 1 USDC)
stellar contract invoke --id "$roles" --source "$IDENTITY" --network mainnet -- init --platform "$ADDR" >/dev/null
stellar contract invoke --id "$fee" --source "$IDENTITY" --network mainnet -- init --platform "$ADDR" --usdc "$usdc_sac" --xlm_fee 5000000 --usdc_fee 1000000 >/dev/null
stellar contract invoke --id "$cnn" --source "$IDENTITY" --network mainnet -- init --platform "$ADDR" >/dev/null
stellar contract invoke --id "$cert" --source "$IDENTITY" --network mainnet -- init --platform "$ADDR" --validator "$ADDR" --fee_collector "$fee" --connoisseur "$cnn" >/dev/null
stellar contract invoke --id "$nft" --source "$IDENTITY" --network mainnet -- init --admin "$ADDR" >/dev/null
stellar contract invoke --id "$part" --source "$IDENTITY" --network mainnet -- init --admin "$ADDR" >/dev/null
stellar contract invoke --id "$mkt" --source "$IDENTITY" --network mainnet -- init --admin "$ADDR" --participation "$part" >/dev/null
stellar contract invoke --id "$dist" --source "$IDENTITY" --network mainnet -- init --admin "$ADDR" --participation "$part" >/dev/null

python3 - <<PY
import json, datetime
out = {
  "network": "mainnet",
  "deployer": "$ADDR",
  "deployedAt": datetime.datetime.utcnow().isoformat() + "Z",
  "usdcIssuer": "$USDC_ISSUER",
  "usdcSac": "$usdc_sac",
  "xlmSac": "$xlm_sac",
  "contracts": {
    "artifex_roles": "$roles",
    "artifex_fee_collector": "$fee",
    "artifex_connoisseur": "$cnn",
    "artifex_cert_registry": "$cert",
    "artifex_whole_nft": "$nft",
    "artifex_participation": "$part",
    "artifex_marketplace": "$mkt",
    "artifex_distribution": "$dist",
  },
  "wholeNftSaleV3": {"list_for_sale": True, "buy_sale": True, "cancel_sale": True},
}
path = "$OUT_DIR/mainnet.json"
with open(path, "w", encoding="utf-8") as f:
  json.dump(out, f, indent=2)
print("Wrote", path)
PY

echo "=== After mainnet deploy ==="
echo "1. Commit deployments/mainnet.json"
echo "2. Fill artifex-api registry mainnet-v3; tag Harbor images"
echo "3. Run orch scripts/mainnet-cutover.sh after Joel confirms + funding"
