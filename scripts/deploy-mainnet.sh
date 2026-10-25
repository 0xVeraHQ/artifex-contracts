#!/usr/bin/env bash
# Deploy Artifex Soroban contracts to Stellar MAINNET.
# HARD STOP: refuses unless ARTIFEX_CONFIRM_MAINNET=JOEL_CONFIRMED.
# Does NOT Friendbot-fund. See deploy-mainnet.ps1 for the Windows twin.
#
# Builds WASM from THIS tree (stellar contract build). Requires main that includes
# living-author propose=sole-signature in artifex_cert_registry (merged #8) so
# mainnet cert_registry is NOT the broken empty-signed propose.
# DO NOT run from a pre-#8 checkout (e.g. unrebased PR #7 head).
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
# Canonical mainnet SAC IDs — abort if stellar CLI resolves anything else.
EXPECTED_USDC_SAC="CCW67TSZV3SSS2HXMBQ5JFGCKJNXKZM7UQUWUZPUTHXSTZLEO7SJMI75"
EXPECTED_XLM_SAC="CAS3J7GYLGXMF6TDJBBYYSE3HQ6BBSMLNUQ34T6TZMYMW2EVH34XOWMA"
usdc_sac="$(stellar contract id asset --asset "USDC:$USDC_ISSUER" --network mainnet | tail -n1 | tr -d '\r')"
xlm_sac="$(stellar contract id asset --asset native --network mainnet | tail -n1 | tr -d '\r')"
echo "USDC SAC (mainnet): $usdc_sac"
echo "XLM SAC (mainnet): $xlm_sac"

assert_eq() {
  local name="$1" got="$2" want="$3"
  got="$(printf '%s' "$got" | tr -d '\r"[:space:]')"
  want="$(printf '%s' "$want" | tr -d '\r"[:space:]')"
  if [[ "$got" != "$want" ]]; then
    echo "ERROR: $name mismatch: got=$got want=$want" >&2
    exit 1
  fi
  echo "OK $name=$got"
}

assert_eq "usdc_sac" "$usdc_sac" "$EXPECTED_USDC_SAC"
assert_eq "xlm_sac" "$xlm_sac" "$EXPECTED_XLM_SAC"

# Mainnet symbolic fees (Joel): 0.5 XLM / 0.1 USDC (not Testnet 5 XLM / 1 USDC)
stellar contract invoke --id "$roles" --source "$IDENTITY" --network mainnet -- init --platform "$ADDR" >/dev/null
stellar contract invoke --id "$fee" --source "$IDENTITY" --network mainnet -- init --platform "$ADDR" --usdc "$usdc_sac" --xlm_fee 5000000 --usdc_fee 1000000 >/dev/null
stellar contract invoke --id "$cnn" --source "$IDENTITY" --network mainnet -- init --platform "$ADDR" >/dev/null
stellar contract invoke --id "$cert" --source "$IDENTITY" --network mainnet -- init --platform "$ADDR" --validator "$ADDR" --fee_collector "$fee" --connoisseur "$cnn" >/dev/null
# Required: fee_collector.consume_deposit only accepts this consumer. Skipping leaves finalize broken.
stellar contract invoke --id "$fee" --source "$IDENTITY" --network mainnet -- set_consumer --caller "$ADDR" --consumer "$cert" >/dev/null
stellar contract invoke --id "$nft" --source "$IDENTITY" --network mainnet -- init --admin "$ADDR" >/dev/null
stellar contract invoke --id "$part" --source "$IDENTITY" --network mainnet -- init --admin "$ADDR" >/dev/null
stellar contract invoke --id "$mkt" --source "$IDENTITY" --network mainnet -- init --admin "$ADDR" --participation "$part" >/dev/null
stellar contract invoke --id "$dist" --source "$IDENTITY" --network mainnet -- init --admin "$ADDR" --participation "$part" >/dev/null

# ---------------------------------------------------------------------------
# Fail-closed post-deploy wiring verify (Testnet Auth InvalidAction class)
# ---------------------------------------------------------------------------
echo "=== Post-deploy wiring verify (abort on mismatch) ==="
got_consumer="$(stellar contract invoke --id "$fee" --source "$IDENTITY" --network mainnet --send=no -- get_consumer | tail -n1 | tr -d '\r')"
if [[ -z "$got_consumer" || "$got_consumer" == "null" ]]; then
  echo "ERROR: fee_collector Consumer unset after set_consumer" >&2
  exit 1
fi
assert_eq "fee_collector.Consumer" "$got_consumer" "$cert"

got_platform="$(stellar contract invoke --id "$roles" --source "$IDENTITY" --network mainnet --send=no -- platform | tail -n1 | tr -d '\r')"
assert_eq "roles.platform" "$got_platform" "$ADDR"

got_fees="$(stellar contract invoke --id "$fee" --source "$IDENTITY" --network mainnet --send=no -- fees | tr -d '\r[:space:]')"
if [[ "$got_fees" != '["5000000","1000000"]' ]]; then
  echo "ERROR: fee_collector.fees mismatch: got=$got_fees want=[\"5000000\",\"1000000\"]" >&2
  exit 1
fi
echo "OK fee_collector.fees=$got_fees"

echo "=== finalize_cert simulation probe (catches wrong Consumer) ==="
PROBE_PIECE="$(openssl rand -hex 32 2>/dev/null || python3 -c 'import os; print(os.urandom(32).hex())')"
CLAIM_HASH="$(openssl rand -hex 32 2>/dev/null || python3 -c 'import os; print(os.urandom(32).hex())')"
stellar contract invoke --id "$cert" --source "$IDENTITY" --network mainnet -- \
  propose_cert \
  --proposer "$ADDR" \
  --piece_id "$PROBE_PIECE" \
  --claim_hash "$CLAIM_HASH" \
  --cert_type 1 \
  --required "[ \"$ADDR\" ]" >/dev/null
stellar contract invoke --id "$fee" --source "$IDENTITY" --network mainnet -- \
  deposit_xlm --payer "$ADDR" --xlm "$xlm_sac" --piece_id "$PROBE_PIECE" >/dev/null

set +e
fin_out="$(stellar contract invoke --id "$cert" --source "$IDENTITY" --network mainnet --send=no -- \
  finalize_cert --caller "$ADDR" --piece_id "$PROBE_PIECE" 2>&1)"
fin_rc=$?
set -e
if [[ $fin_rc -ne 0 ]] || printf '%s' "$fin_out" | grep -qiE 'InvalidAction|Auth'; then
  echo "ERROR: finalize_cert simulation failed (rc=$fin_rc) — likely wrong Consumer (Auth InvalidAction class)" >&2
  printf '%s\n' "$fin_out" >&2
  stellar contract invoke --id "$fee" --source "$IDENTITY" --network mainnet -- \
    refund_deposit --caller "$ADDR" --piece_id "$PROBE_PIECE" --xlm "$xlm_sac" >/dev/null 2>&1 || true
  exit 1
fi
echo "OK finalize_cert simulation"

stellar contract invoke --id "$fee" --source "$IDENTITY" --network mainnet -- \
  refund_deposit --caller "$ADDR" --piece_id "$PROBE_PIECE" --xlm "$xlm_sac" >/dev/null
echo "OK probe deposit refunded (finalize not submitted)"
echo "=== wiring verify passed ==="

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
  "wiringVerified": True,
  "feeCollectorConsumer": "$cert",
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
