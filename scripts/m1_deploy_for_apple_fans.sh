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

# Copy binaries to docker container
docker cp artifacts/junoswap.wasm cosmwasm:/junoswap.wasm
docker cp artifacts/factory.wasm cosmwasm:/factory.wasm
docker cp scripts/cw20_stakeable.wasm cosmwasm:/cw20_stakeable.wasm

# Sleep while waiting for chain to post genesis block
sleep 15

echo "Address to deploy contracts: $1"
echo "TX Flags: $TXFLAG"


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
CW20_CONTRACT=juno14hj2tavq8fpesdwxxcu44rty3hh90vhujrvcmstl4zr3txmfvw9skjuwg8
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
SWAP_1_CONTRACT=juno1nc5tatafv6eyq7llkr2gv50ff9e22mnf70qgjlv737ktmt4eswrq68ev2p
echo $SWAP_1_CONTRACT

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
CW20_CONTRACT_2=juno1yw4xvtc43me9scqfr2jr2gzvcxd3a9y4eq7gaukreugw2yd2f8ts9z8cq8
echo $CW20_CONTRACT_2

# Initialize factory contract
SWAP_2_INIT='{
    "token1_denom": {"native": "ujuno"},
    "token2_denom": {"cw20": "'"$CW20_CONTRACT_2"'"},
    "lp_token_code_id": '$CW20_CODE'
}'

echo "$SWAP_2_INIT"
echo xxxxxxxxx | $BINARY tx wasm instantiate $JUNOSWAP_CODE "$SWAP_2_INIT" --from "validator" --label "swap_2" $TXFLAG
SWAP_2_CONTRACT=juno1hulx7cgvpfcvg83wk5h96sedqgn72n026w6nl47uht554xhvj9ns263mx9

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
CW20_CONTRACT_3=juno1wl59k23zngj34l7d42y9yltask7rjlnxgccawc7ltrknp6n52fpsndlmj0
echo $CW20_CONTRACT_3

# Initialize factory contract
SWAP_3_INIT='{
    "token1_denom": {"native": "ujuno"},
    "token2_denom": {"cw20": "'"$CW20_CONTRACT_3"'"},
    "lp_token_code_id": '$CW20_CODE'
}'

echo "$SWAP_3_INIT"
echo xxxxxxxxx | $BINARY tx wasm instantiate $JUNOSWAP_CODE "$SWAP_3_INIT" --from "validator" --label "swap_2" $TXFLAG
SWAP_3_CONTRACT=juno182nff4ttmvshn6yjlqj5czapfcav9434l2qzz8aahf5pxnyd33tstk25ur

echo "CRAB cw20 contract 1"
echo $CW20_CONTRACT
echo "CRAB Swap contract 1"
echo $SWAP_1_CONTRACT
echo "DAO cw20 contract 2"
echo $CW20_CONTRACT_2
echo "DAO Swap contract 2"
echo $SWAP_2_CONTRACT
echo "POOD cw20 contract 2"
echo $CW20_CONTRACT_3
echo "POOD Swap contract 2"
echo $SWAP_3_CONTRACT
