# Crustacean Swap

This contract is an automatic market maker (AMM) heavily inspired by Uniswap v1 for the cosmwasm smart contract engine.

This project is currently in beta and is unaudited so please use at your own risk.

This contract allows you to swap native cosmos coins for cw20 tokens. Liquidity providers can add liquidity to the market and receive a 0.03% fee on every transaction.

# Usage

The following instructions are written for the Juno testnet, however this contract can be run on any cosmwasm enabled chain.

## Deploy
```junod tx wasm instantiate 20 '{"native_denom": "<native_denom>", "token_address":"<cw20_contract_address>", "token_denom": "<token_denom>"}'  --from <key> --label="<label>" --gas="auto" --chain-id="lucina"```

## Execute Messages

### Add Liquidity

This message adds liquidity to the pool and give the caller proportional ownership of pool funds. Funds need to be deposited at the current ratio of the pools reserves, ie if the pool currently has 100 native tokens and 300 cw20 tokens the caller needs to deposit at a ratio of 1 to 3. Max token should be set a little higher than expected in case there are any changes in the pool reserves.

```junod tx wasm execute <cw20_contract_address> '{"increase_allowance":{"amount":"<max_token>","spender":"<contract_address>"}}' --from <key> --chain-id="lucina"```

```junod tx wasm execute <contract_address> '{"add_liquidity":{"max_token":"<max_token>","min_liquidity":"<min_liquidity>"}}' --from <key> --amount "<native_amount>" --chain-id="lucina"```

### Remove Liquidity

This removes liquidity from the pool and returns it to the owner. Current liquidity owner ship can be seen with the balance query below. `min_native` and `min_token` are used to ensure the pool reserves do no unexpectedly change. Set both values to 1 if you want to guarantee the message is executed.

```junod tx wasm execute <contract_address> '{"remove_liquidity":{"amount":"<liquidity_amount>","min_native":"<min_native>","min_token":"<min_native>"}}' --from <key> --chain-id="lucina"```

### Swap Native For Token

This swaps the native token for the cw20 token. Use the price query below to estimate the price before executing this message. `min_token` is used to set limit on acceptable price for the swap.

```junod tx wasm execute <contract_address> '{"swap_native_for_token":{"min_token":"<min_token>"}}' --from <key> --amount "<native_amount>" --chain-id="lucina"```

### Swap Token For Native

This swaps the native token for the cw20 token. First, the swap contract must be given an allowance of the cw20 token. Use the price query below to estimate the price before executing this message. `token_amount` should be the amount of allowance given to the swap contract. `min_native` is used to set limit on acceptable price for the swap.

```junod tx wasm execute <cw20_contract_address> '{"increase_allowance":{"amount":"<token_amount>","spender":"<contract_address>"}}' --from <key> --chain-id="lucina"```

```junod tx wasm execute <contract_address> '{"swap_token_for_native":{"min_native":"<min_native>", "token_amount":"<token_amount>"}}' --from bob --chain-id="lucina"```

### Advanced Features

All execute messages can also be given an expiration for greater security. This is recommended in a production environment. Exact specifications for the expiration field can be viewed in `schema/execute_msg.json`.

## Query Messages

### Info

This returns information about the assets in the pool and the size of the reserves.

```junod query wasm contract-state smart <contract-address> '{"info":{}}' --chain-id="lucina"```

### Native For Token Price

This returns the current swap result for the desired native token amount.

```junod query wasm contract-state smart <contract-address> '{"native_for_token_price":{"native_amount":"<native-amount>"}}' --chain-id="lucina"```

### Token For Native Price

This returns the current swap result for the desired cw20 token amount.

```junod query wasm contract-state smart <contract-address> '{"token_for_native_price":{"token_amount":"<token-amount>"}}' --chain-id="lucina"```

### Balance

This returns the current liquidity token balance for the address.

```junod query wasm contract-state smart <contract-address> '{"balance":{"address":"<address>"}}' --chain-id="lucina"```
