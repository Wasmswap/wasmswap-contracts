#!/bin/bash

## CONFIG
BINARY='fetchd'
DENOM='atestfet'
CHAIN_ID='test'
RPC='http://localhost:26657/'
# CHAIN_ID='capricorn-1'
# RPC='https://rpc-capricorn.fetch.ai:443'
BLOCK_SLEEP=5
TXFLAG="--gas auto --chain-id $CHAIN_ID --node $RPC -y"
FETCHMNEMONIC="boat leave enrich glare into second this model appear owner strong tail perfect fringe best still soup clap betray rigid bleak return minimum goddess"
FROM_ACCOUNT=alice

# Add key
echo $FETCHMNEMONIC | $BINARY keys add $FROM_ACCOUNT --recover
FROM_ADDRESS=$(fetchd keys show -a $FROM_ACCOUNT)

# If using remote testnet, add funds from faucet to this address: https://explore-capricorn.fetch.ai/

printf "\nStore cw20 contract code\n\n"
$BINARY tx wasm store "cw20_base.wasm" --from $FROM_ACCOUNT $TXFLAG --fees 9000000000000000atestfet
CW20_CODE=1 #375
sleep $BLOCK_SLEEP

printf "\nInstantiate cw20 contract\n\n"
CW20_INIT='{
    "name": "Crab Coin",
    "symbol": "CRAB",
    "decimals": 6,
    "initial_balances": [{"address":"'$FROM_ADDRESS'","amount":"1000000000"}]
}'
$BINARY tx wasm instantiate $CW20_CODE "$CW20_INIT" --from $FROM_ACCOUNT --label "token" $TXFLAG --fees 800000000000000atestfet 
sleep $BLOCK_SLEEP

printf "\nGet cw20 contract address\n\n"
CW20_CONTRACT=$($BINARY q wasm list-contract-by-code $CW20_CODE --output json | jq -r '.contracts[-1]')
echo $CW20_CONTRACT

printf "\nStore liquidity pool factory contract code\n\n"
$BINARY tx wasm store "wasmswap.wasm" --from $FROM_ACCOUNT $TXFLAG --fees 10000000000000000atestfet
WASMSWAP_CODE=2 #376
sleep $BLOCK_SLEEP

printf "\nInitialize factory contract\n\n"
SWAP_1_INIT='{
    "token1_denom": {"native": "atestfet"},
    "token2_denom": {"cw20": "'$CW20_CONTRACT'"},
    "lp_token_code_id": '"$CW20_CODE"'
}'

echo "$SWAP_1_INIT"
$BINARY tx wasm instantiate $WASMSWAP_CODE "$SWAP_1_INIT" --from $FROM_ACCOUNT --label "swap_1" $TXFLAG --fees 13000000000000000atestfet
sleep $BLOCK_SLEEP
SWAP_1_CONTRACT=$($BINARY q wasm list-contract-by-code $WASMSWAP_CODE --output json | jq -r '.contracts[-1]')
echo $SWAP_1_CONTRACT

printf "\nApprove cw20 contract to spend of tokens\n\n"
LIQUIDITY_AMOUNT=100000000
$BINARY tx wasm execute $CW20_CONTRACT '{"increase_allowance":{"amount":"'$LIQUIDITY_AMOUNT'","spender":"'"$SWAP_1_CONTRACT"'"}}' --from $FROM_ACCOUNT $TXFLAG --fees 1900000000000000atestfet
sleep $BLOCK_SLEEP

printf "\nAdd liquidity to FET-Token pair\n\n"
$BINARY tx wasm execute $SWAP_1_CONTRACT '{"add_liquidity":{"token1_amount":"'$LIQUIDITY_AMOUNT'","max_token2":"'$LIQUIDITY_AMOUNT'","min_liquidity":"1"}}' --from $FROM_ACCOUNT --amount ${LIQUIDITY_AMOUNT}atestfet $TXFLAG --fees 1900000000000000atestfet
sleep $BLOCK_SLEEP

printf "\nQuery contract info\n\n"
$BINARY query wasm contract-state smart $SWAP_1_CONTRACT '{"info":{}}'

printf "\nQuery token price for swap\n\n"
INPUT_AMOUNT=100
$BINARY query wasm contract-state smart $SWAP_1_CONTRACT '{"token1_for_token2_price":{"token1_amount":"'$INPUT_AMOUNT'"}}'
MIN_OUTPUT=99

printf "\nExecute swap (swap FET in exchange for tokens)\n\n"
$BINARY tx wasm execute $SWAP_1_CONTRACT '{"swap":{"input_token": "Token1","input_amount":"'$INPUT_AMOUNT'","min_output":"'$MIN_OUTPUT'"}}' --from $FROM_ACCOUNT $TXFLAG --amount ${INPUT_AMOUNT}atestfet --fees 1200000000000000atestfet
sleep $BLOCK_SLEEP

printf "\nQuery contract info\n\n"
$BINARY query wasm contract-state smart $SWAP_1_CONTRACT '{"info":{}}'
