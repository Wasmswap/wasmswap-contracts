# WasmSwap

This contract is an automatic market maker (AMM) heavily inspired by Uniswap v1 for the cosmwasm smart contract engine.

This project is currently in beta and is unaudited so please use at your own risk.

This contract allows you to swap tokens. Liquidity providers can add liquidity to the market and receive a 0.03% fee on every transaction.

# Instantiation

The contract can be instantiated with the following messages

```
{
    "token1_denom": {"native": "<DENOM>"},
    "token2_denom": {"cw20": "<CONTRACT_ADDRESS>"},
    "lp_token_code_id": '<CW20_CODE_ID>'
}
```

Token denom can be either `native` for tokens tracked by the bank module (including IBC assets) or `cw20` for cw20 tokens. `native` tokens have a denom string and `cw20` tokens have a contract address. `CW20_CODE_ID` is the code id for a basic cw20 binary.

# Messages

### Add Liquidity

Allows a user to add liquidity to the pool.

### Remove Liquidity

Allows a user to remove liquidity from the pool.

### Swap

Swap one asset for the other

### Pass Through Swap

Execute a multi contract swap where A is swapped for B and then B is sent to another contract where it is swapped for C.

### Swap And Send To

Execute a swap and send the new asset to the given recipient. This is mostly used for `PassThroughSwaps`.
