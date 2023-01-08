#!/bin/bash

# Run this from the root repo directory

## CONFIG
# NOTE: you will need to update these to deploy on different network
BINARY='junod'
DENOM='ujunox'
CHAIN_ID='uni-5'
# RPC='http://localhost:26657/'
# REST='http://localhost:1317/'
# TXFLAG="--gas-prices 0.025$DENOM --gas auto --gas-adjustment 1.3 -y -b block --chain-id $CHAIN_ID --node $RPC"
TXFLAG="--gas-prices 0.1$DENOM --gas auto --gas-adjustment 1.3 -y -b block --chain-id $CHAIN_ID"
# QFLAG="--chain-id $CHAIN_ID --node $RPC"
QFLAG="--chain-id $CHAIN_ID"
TESTER_MNEMONIC="siren window salt bullet cream letter huge satoshi fade shiver permit offer happy immense wage fitness goose usual aim hammer clap about super trend"

if [ "$1" = "" ]
then
  echo "Usage: $0 2 args required, LP address is missing!"
  exit
fi

# LP Data, like pool address, the cw20 addresses.
POOL_ADDR=$1
LP_RESPONSE=$($BINARY q wasm contract-state smart $POOL_ADDR '{"info":{}}' --output json $QFLAG | jq -r '.data')
TOKEN_1_ADDR=$(echo $LP_RESPONSE | jq -r '.token1_denom | select(.cw20 != null) | .cw20')
TOKEN_2_ADDR=$(echo $LP_RESPONSE | jq -r '.token2_denom | select(.cw20 != null) | .cw20')
LP_TOKEN_ADDR=$(echo $LP_RESPONSE | jq -r '.lp_token_address')

# echo $LP_RESPONSE
# echo $TOKEN_1_DENOM $TOKEN_2_DENOM $LP_TOKEN_ADDR

# TODO: add a docker with junod installed to do it there instead of locally.

# Read from the mnemonic file line by line
BASE_NAME="ws-tester"
INDEX=0

for INDEX in {1..1}
do
  # Add tester address to junod
  TESTER_NAME=$BASE_NAME"-"$INDEX
  printf "y\n$TESTER_MNEMONIC" | $BINARY keys add $TESTER_NAME --account $INDEX --recover

# We assume only 1 token is cw20, and the 2nd is native for now
  if [ "$TOKEN_1_ADDR" = "" ]
  then
    TOKEN_ADDR=$TOKEN_2_ADDR
  else
    TOKEN_ADDR=$TOKEN_1_ADDR
  fi

  # TODO: How do we fund the accounts? no cli faucet or we can't do swaps if 2 CW20 tokens
  # For native LPs, we can just swap
  $BINARY tx wasm execute $POOL_ADDR '{"swap":{"input_token": "Token2","input_amount":"200000","min_output":"1"}}' --from $TESTER_NAME --amount 200000ujunox $TXFLAG

  # increase allowance
  $BINARY tx wasm execute $TOKEN_ADDR '{"increase_allowance":{"amount":"2000000","spender":"'"$POOL_ADDR"'"}}' --from $TESTER_NAME $TXFLAG

  # Add liquidity
  $BINARY tx wasm execute $POOL_ADDR '{"add_liquidity":{"token1_amount":"100","min_liquidity":"1","max_token2":"131"}}' --from $TESTER_NAME --amount 131ujunox $TXFLAG

  # Increase index
  let INDEX=INDEX+1
done
