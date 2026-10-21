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
./scripts/deploy-testnet.sh   # or scripts/deploy-testnet.ps1
```

Contract IDs written to `deployments/testnet.json` (gitignored secrets; IDs are public).

**Version:** see `VERSION` (`0.1.0`). After deploy, tag `main` with `vX.Y.Z` — release.yml validates; no Harbor image.

Contract IDs: `deployments/testnet.json`. API registry is runtime source of truth (`testnet.api.env` only sets `STELLAR_NETWORK`).
