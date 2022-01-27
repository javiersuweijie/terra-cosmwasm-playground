# terra-cosmwasm-playground
Cosmwasm contracts written to learn to write more

> Warning: Nothing in this repo should be used as good examples nor used in production.

# How to run the contracts

## Dependencies

1. Localterra
2. Rust
3. `cosmwasm/workspace-optimizer` and `cosmwasm/rust-optimizer` to compile the contracts

## Running integration tests

1. Compile the contracts with the cosmwasm optimizer
2. Each contract should have the scripts to deploy to localterra found under `./scripts`
3. Run localterra and run the `main.ts` scripts.

## Credits

- Deployment scripts: https://github.com/larry0x/spacecamp-2021-workshop
- Asset packages: https://github.com/terraswap/terraswap
