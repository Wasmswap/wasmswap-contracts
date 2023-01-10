#!/bin/bash

# Run this from the scripts directory

# First argument is the Pool address, second argument is the staking contract of the same pool address.
# Example: `./add_liquidity.sh juno1gjk9gva6dfs254qvlthw6ldqu3etp3wf2854n2ln7szepzvrggys89w3yn juno1paazsk7v2sx8tdjgw9ud22mg6hrawqwd27tjrv4nex9v529yzphsglem68`

## CONFIG
BINARY='docker exec -i cosmwasm junod'
DENOM='ujunox'
CHAIN_ID='uni-5'
RPC='https://rpc.uni.junonetwork.io:443'
TXFLAG="--gas-prices 0.1$DENOM --gas auto --gas-adjustment 1.3 -y -b block --chain-id $CHAIN_ID --node $RPC"
QFLAG="--chain-id $CHAIN_ID --node $RPC"
TESTER_MNEMONIC='siren window salt bullet cream letter huge satoshi fade shiver permit offer happy immense wage fitness goose usual aim hammer clap about super trend'
FAUCET_ADDR='juno12gqyglvj4rdg4kcc292s8z7u25d3s4tuph98y8'
KILL_DOCKER='docker kill cosmwasm'

if [ "$1" = "" ]
then
  echo "Usage: $0 2 args required, 1st argument LP address is missing!"
  exit
elif [ "$2" = "" ]
then
  echo "Usage: $0 2 args required, 2nd argument Staking address is missing!"
  exit
fi

# Start docker with junod installed
$KILL_DOCKER &>/dev/null

docker run --rm -d -t --name cosmwasm \
    --mount type=volume,source=junod_data,target=/root \
    --platform linux/amd64 \
    ghcr.io/cosmoscontracts/juno:v11.0.0 sh

# Try to delete faucet if already exists
$BINARY keys delete faucet -y >/dev/null &>/dev/null

# Add faucet wallet
echo $TESTER_MNEMONIC | $BINARY keys add faucet --account 1 --recover >/dev/null

# LP Data, like pool address, staking address, the cw20 addresses.
POOL_ADDR=$1
STAKING_ADDR=$2

LP_RESPONSE=$($BINARY q wasm contract-state smart $POOL_ADDR '{"info":{}}' --output json $QFLAG | jq -r '.data')
TOKEN_1_ADDR=$(echo $LP_RESPONSE | jq -r '.token1_denom | select(.cw20 != null) | .cw20')
TOKEN_2_ADDR=$(echo $LP_RESPONSE | jq -r '.token2_denom | select(.cw20 != null) | .cw20')
LP_TOKEN_ADDR=$(echo $LP_RESPONSE | jq -r '.lp_token_address')

# echo $LP_RESPONSE
# echo $TOKEN_1_ADDR $TOKEN_2_ADDR $LP_TOKEN_ADDR

BASE_NAME="ws-tester"

# NOTE: Important to start from index 2, because index 0 is our manual tester, index 1 is our faucet
for INDEX in {2..2}
do
  TESTER_NAME=$BASE_NAME"-"$INDEX

  # Add tester address to junod
  $BINARY keys delete $TESTER_NAME -y &>/dev/null
  echo "$TESTER_MNEMONIC" | $BINARY keys add $TESTER_NAME --account $INDEX --recover >/dev/null
  TESTER_ADDR=$($BINARY keys show $TESTER_NAME --address)

  # Use bank to fund wallet from faucet (for gas and pool)
  $BINARY tx bank send faucet $TESTER_ADDR 100000ujunox $TXFLAG >/dev/null || exit

  if [ -n "$TOKEN_1_ADDR" ]
  then
    # CW20 token, use transfer to fund wallet from faucet
    $BINARY tx wasm execute $TOKEN_1_ADDR '{"transfer":{"recipient":"'"$TESTER_ADDR"'","amount":"150"}}' --from faucet $TXFLAG >/dev/null || exit
    TOKEN_ADDR=$TOKEN_1_ADDR
  fi

  if [ -n "$TOKEN_2_ADDR" ]
  then
    # CW20 token, use transfer to fund wallet from faucet
    $BINARY tx wasm execute $TOKEN_2_ADDR '{"transfer":{"recipient":"'"$TESTER_ADDR"'","amount":"150"}}' --from faucet $TXFLAG >/dev/null || exit
    TOKEN_ADDR=$TOKEN_2_ADDR
  fi

  # increase allowance
  $BINARY tx wasm execute $TOKEN_ADDR '{"increase_allowance":{"amount":"20000","spender":"'"$POOL_ADDR"'"}}' --from $TESTER_NAME $TXFLAG || exit

  # Add liquidity
  $BINARY tx wasm execute $POOL_ADDR '{"add_liquidity":{"token1_amount":"100","min_liquidity":"1","max_token2":"131"}}' --from $TESTER_NAME --amount 131ujunox $TXFLAG || exit

  # Stake
  staking_msg=`echo '{"stake":{}}' | base64`
  $BINARY tx wasm execute $LP_TOKEN_ADDR '{"send":{"contract":"'"$STAKING_ADDR"'","amount":"90","msg":"'"$staking_msg"'"}}' --from $TESTER_NAME $TXFLAG

  # Increase index
  let INDEX=INDEX+1

  echo $TESTER_NAME" finished successfully!"
done

$KILL_DOCKER


# Swap msg for native denom
# $BINARY tx wasm execute $POOL_ADDR '{"swap":{"input_token": "Token2","input_amount":"2000000","min_output":"1"}}' --from faucet --amount 2000000ujunox $TXFLAG
