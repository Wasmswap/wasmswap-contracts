#!/bin/bash

# Run this from the root repo directory

## CONFIG
# NOTE: you will need to update these to deploy on different network
BINARY='docker exec -i cosmwasm junod'
DENOM='ujuno'
CHAIN_ID='testing'
RPC='http://localhost:26657/'
REST='http://localhost:1317/'
TXFLAG="--gas-prices 0.01$DENOM --gas auto --gas-adjustment 1.3 -y -b block --chain-id $CHAIN_ID --node $RPC"

if [ "$1" = "" ]
then
  echo "Usage: $0 1 arg required, wasm address. See \"Deploying in a development environment\" in README."
  exit
fi

# Deploy junod in Docker
docker kill cosmwasm

docker volume rm -f junod_data

# Run junod setup script
docker run --rm -it \
    -e STAKE_TOKEN=$DENOM \
    -e PASSWORD=xxxxxxxxx \
    --mount type=volume,source=junod_data,target=/root \
    ghcr.io/cosmoscontracts/juno:pr-105 /opt/setup_junod.sh $1

# Add custom app.toml to junod_data volume
docker run -v junod_data:/root --name helper busybox true
docker cp docker/app.toml helper:/root/.juno/config/app.toml
docker cp docker/config.toml helper:/root/.juno/config/config.toml
docker rm helper

# Start junod
docker run --rm -d --name cosmwasm -p 26657:26657 -p 26656:26656 -p 1317:1317 \
    --mount type=volume,source=junod_data,target=/root \
    ghcr.io/cosmoscontracts/juno:pr-105 /opt/run_junod.sh

# Compile code
docker run --rm -v "$(pwd)":/code \
  --mount type=volume,source="$(basename "$(pwd)")_cache",target=/code/target \
  --mount type=volume,source=registry_cache,target=/usr/local/cargo/registry \
  cosmwasm/rust-optimizer:0.12.3

# Copy binaries to docker container
docker cp artifacts/junoswap.wasm cosmwasm:/junoswap.wasm
docker cp artifacts/factory.wasm cosmwasm:/factory.wasm
docker cp scripts/cw20_stakeable.wasm cosmwasm:/cw20_stakeable.wasm

# Sleep while waiting for chain to post genesis block
sleep 3

echo "Address to deploy contracts: $1"
echo "TX Flags: $TXFLAG"

(echo "siren window salt bullet cream letter huge satoshi fade shiver permit offer happy immense wage fitness goose usual aim hammer clap about super trend") | $BINARY keys add test --recover

#### CW20-GOV ####
# Upload cw20 contract code
echo xxxxxxxxx | $BINARY tx wasm store "/cw20_stakeable.wasm" --from validator $TXFLAG
CW20_CODE=1

# Instantiate cw20 contract
CW20_INIT='{
 "cw20_base" : {
    "name": "Crab Coin",
    "symbol": "CRAB",
    "decimals": 6,
    "initial_balances": [{"address":"'"$1"'","amount":"1000000000"}]
  }
}'
echo "$CW20_INIT"
echo xxxxxxxxx | $BINARY tx wasm instantiate $CW20_CODE "$CW20_INIT" --from "validator" --label "token" $TXFLAG

# Get cw20 contract address
CW20_CONTRACT=$($BINARY q wasm list-contract-by-code $CW20_CODE --output json | jq -r '.contracts[-1]')
echo $CW20_CONTRACT

# Upload cw-dao contract code
echo xxxxxxxxx | $BINARY tx wasm store "/junoswap.wasm" --from validator $TXFLAG
JUNOSWAP_CODE=2

echo $JUNOSWAP_CODE

# Initialize factory contract
SWAP_1_INIT='{
    "token1_denom": {"native": "ujuno"},
    "token2_denom": {"cw20": "'"$CW20_CONTRACT"'"},
    "lp_token_code_id": '$CW20_CODE'
}'

echo "$SWAP_1_INIT"
echo xxxxxxxxx | $BINARY tx wasm instantiate $JUNOSWAP_CODE "$SWAP_1_INIT" --from "validator" --label "swap_1" $TXFLAG
SWAP_1_CONTRACT=$($BINARY q wasm list-contract-by-code $JUNOSWAP_CODE --output json | jq -r '.contracts[-1]')
echo $SWAP_1_CONTRACT
$BINARY tx wasm execute $CW20_CONTRACT '{"increase_allowance":{"amount":"100000000","spender":"'"$SWAP_1_CONTRACT"'"}}' --from test $TXFLAG
$BINARY tx wasm execute $SWAP_1_CONTRACT '{"add_liquidity":{"token1_amount":"100000000","max_token2":"100000000","min_liquidity":"1"}}' --from test --amount "100000000ujuno" $TXFLAG

# Instantiate cw20 contract
CW20_INIT_2='{
 "cw20_base" : {
    "name": "DAO Coin",
    "symbol": "DAO",
    "decimals": 6,
    "initial_balances": [{"address":"'"$1"'","amount":"1000000000"}]
  }
}'
echo "$CW20_INIT_2"
echo xxxxxxxxx | $BINARY tx wasm instantiate $CW20_CODE "$CW20_INIT_2" --from "validator" --label "token" $TXFLAG

# Get cw20 contract address
CW20_CONTRACT_2=$($BINARY q wasm list-contract-by-code $CW20_CODE --output json | jq -r '.contracts[-1]')
echo $CW20_CONTRACT_2

# Initialize factory contract
SWAP_2_INIT='{
    "token1_denom": {"native": "ujuno"},
    "token2_denom": {"cw20": "'"$CW20_CONTRACT_2"'"},
    "lp_token_code_id": '$CW20_CODE'
}'

echo "$SWAP_2_INIT"
echo xxxxxxxxx | $BINARY tx wasm instantiate $JUNOSWAP_CODE "$SWAP_2_INIT" --from "validator" --label "swap_2" $TXFLAG
SWAP_2_CONTRACT=$($BINARY q wasm list-contract-by-code $JUNOSWAP_CODE --output json | jq -r '.contracts[-1]')
$BINARY tx wasm execute $CW20_CONTRACT_2 '{"increase_allowance":{"amount":"100000000","spender":"'"$SWAP_2_CONTRACT"'"}}' --from test $TXFLAG
$BINARY tx wasm execute $SWAP_2_CONTRACT '{"add_liquidity":{"token1_amount":"100000000","max_token2":"100000000","min_liquidity":"1"}}' --from test --amount "100000000ujuno" $TXFLAG

# Instantiate cw20 contract
CW20_INIT_3='{
 "cw20_base" : {
    "name": "POOD Coin",
    "symbol": "POOD",
    "decimals": 6,
    "initial_balances": [{"address":"'"$1"'","amount":"1000000000"}]
  }
}'
echo "$CW20_INIT_3"
echo xxxxxxxxx | $BINARY tx wasm instantiate $CW20_CODE "$CW20_INIT_3" --from "validator" --label "token" $TXFLAG

# Get cw20 contract address
CW20_CONTRACT_3=$($BINARY q wasm list-contract-by-code $CW20_CODE --output json | jq -r '.contracts[-1]')
echo $CW20_CONTRACT_3

# Initialize factory contract
SWAP_3_INIT='{
    "token1_denom": {"native": "ujuno"},
    "token2_denom": {"cw20": "'"$CW20_CONTRACT_3"'"},
    "lp_token_code_id": '$CW20_CODE'
}'

echo "$SWAP_3_INIT"
echo xxxxxxxxx | $BINARY tx wasm instantiate $JUNOSWAP_CODE "$SWAP_3_INIT" --from "validator" --label "swap_3" $TXFLAG
SWAP_3_CONTRACT=$($BINARY q wasm list-contract-by-code $JUNOSWAP_CODE --output json | jq -r '.contracts[-1]')

$BINARY tx wasm execute $CW20_CONTRACT_3 '{"increase_allowance":{"amount":"100000000","spender":"'"$SWAP_3_CONTRACT"'"}}' --from test $TXFLAG
$BINARY tx wasm execute $SWAP_3_CONTRACT '{"add_liquidity":{"token1_amount":"100000000","max_token2":"100000000","min_liquidity":"1"}}' --from test --amount "100000000ujuno" $TXFLAG


# Initialize factory contract
SWAP_4_INIT='{
    "token1_denom": {"native": "ujuno"},
    "token2_denom": {"native": "ucosm"},
    "lp_token_code_id": '$CW20_CODE'
}'

echo "$SWAP_4_INIT"
echo xxxxxxxxx | $BINARY tx wasm instantiate $JUNOSWAP_CODE "$SWAP_4_INIT" --from "validator" --label "swap_4" $TXFLAG
SWAP_4_CONTRACT=$($BINARY q wasm list-contract-by-code $JUNOSWAP_CODE --output json | jq -r '.contracts[-1]')

$BINARY tx wasm execute $SWAP_4_CONTRACT '{"add_liquidity":{"token1_amount":"100000000","max_token2":"100000000","min_liquidity":"1"}}' --from test --amount "100000000ujuno,100000000ucosm" $TXFLAG

echo "CRAB cw20 contract 1"
echo $CW20_CONTRACT
echo "CRAB Swap contract 1"
echo $SWAP_1_CONTRACT
echo "DAO cw20 contract 2"
echo $CW20_CONTRACT_2
echo "DAO Swap contract 2"
echo $SWAP_2_CONTRACT
echo "POOD cw20 contract 3"
echo $CW20_CONTRACT_3
echo "POOD Swap contract 3"
echo $SWAP_3_CONTRACT
echo "COSM SWap contract 4"
echo $SWAP_4_CONTRACT
