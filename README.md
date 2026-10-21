# artifex-contracts

Soroban contracts for Artifex **Scale** hackathon track (Testnet only).

See ADR: `artifex-docs/adr/010-soroban-scale-architecture.md`.

## Contracts

| Crate | Role |
|-------|------|
| `artifex_roles` | Wallet roles |
| `artifex_fee_collector` | Cert fees (XLM / USDC SAC) |
| `artifex_connoisseur` | Connoisseur authorization |
| `artifex_cert_registry` | Multi-sig certificates |
| `artifex_whole_nft` | Whole-work NFT + delivery clause |
| `artifex_participation` | Offers + atomic primary sale |
| `artifex_marketplace` | Secondary escrow + partial fills |
| `artifex_distribution` | Distribution / burn by type |

## Build / test

```sh
# Windows: use GNU toolchain if MSVC link.exe is missing
cargo +stable-x86_64-pc-windows-gnu test

stellar contract build
```

## Deploy

```sh
./scripts/deploy-testnet.sh   # or scripts/deploy-testnet.ps1   # full fresh deploy → deployments/testnet.json
./scripts/deploy-registry-v2.ps1  # keep v1; redeploy fee/cert/participation/mkt/dist → deployments/testnet-v2.json
```

Contract IDs written to `deployments/testnet.json` / `testnet-v2.json` (IDs are public; no secrets).

**Version:** see `VERSION` (`0.1.0`). After deploy, tag `main` with `vX.Y.Z` — release.yml validates; no Harbor image.

API registry (`artifex-api` `registry.json`) is runtime source of truth — copy v2 IDs into `versions.v2` and cut over when wired (`testnet.api.env` only sets `STELLAR_NETWORK`).

### v2 contract API notes (for API/web lead)

| Change | Detail |
|--------|--------|
| `finalize_with_fee(caller, piece_id, fee_asset, xlm)` | Atomic fee+finalize; prefer over `collect_*` → `mark_fee_paid` → `finalize_cert` |
| `deposit_xlm` / `deposit_usdc` + `refund` | Piece-scoped fee escrow; refundable until finalize consumes |
| `create_offer(..., payment_tokens[], unit_prices[], ...)` | Multi-currency accepted set |
| `buy_primary(buyer, offer_id, units, payment_token)` | Charges **only** the buyer-selected accepted token |
