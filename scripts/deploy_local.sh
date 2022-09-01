#!/bin/bash

# Run this from the root repo directory

## CONFIG
# NOTE: you will need to update these to deploy on different network
BINARY='docker exec -i cosmwasm junod'
DENOM='ujuno'
CHAIN_ID='testing'
RPC='http://localhost:26657/'
REST='http://localhost:1317/'
TXFLAG="--gas-prices 0.025$DENOM --gas auto --gas-adjustment 1.3 -y -b block --chain-id $CHAIN_ID --node $RPC"
QFLAG="--chain-id $CHAIN_ID --node $RPC"

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
    ghcr.io/cosmoscontracts/juno:v3.1.0 /opt/setup_junod.sh $1

# Add custom app.toml to junod_data volume
docker run -v junod_data:/root --name helper busybox true
docker cp docker/app.toml helper:/root/.juno/config/app.toml
docker cp docker/config.toml helper:/root/.juno/config/config.toml
docker rm helper

# Start junod
docker run --rm -d --name cosmwasm -p 26657:26657 -p 26656:26656 -p 1317:1317 \
    --mount type=volume,source=junod_data,target=/root \
    --platform linux/amd64 \
    ghcr.io/cosmoscontracts/juno:v2.1.0 /opt/run_junod.sh

# Compile code
docker run --rm -v "$(pwd)":/code \
--mount type=volume,source="$(basename "$(pwd)")_cache",target=/code/target \
--mount type=volume,source=registry_cache,target=/usr/local/cargo/registry \
--platform linux/amd64 \
cosmwasm/rust-optimizer:0.12.3

# Copy binaries to docker container
docker cp artifacts/wasmswap.wasm cosmwasm:/wasmswap.wasm
docker cp scripts/cw20_base.wasm cosmwasm:/cw20_base.wasm
docker cp scripts/stake_cw20.wasm cosmwasm:/stake_cw20.wasm
docker cp scripts/stake_cw20_external_rewards.wasm cosmwasm:/stake_cw20_external_rewards.wasm

# Sleep while waiting for chain to post genesis block
sleep 10

echo "Address to deploy contracts: $1"
echo "TX Flags: $TXFLAG"

(echo "siren window salt bullet cream letter huge satoshi fade shiver permit offer happy immense wage fitness goose usual aim hammer clap about super trend") | $BINARY keys add test --recover

#### CW20-GOV ####
# Upload cw20 contract code
echo xxxxxxxxx | $BINARY tx wasm store "/cw20_base.wasm" --from validator $TXFLAG
CW20_CODE=$($BINARY q wasm list-code --reverse --output json $QFLAG | jq -r '.code_infos[0].code_id')
echo $CW20_CODE


# Instantiate cw20 contract
CW20_INIT='{
    "name": "Crab Coin",
    "symbol": "CRAB",
    "decimals": 6,
    "initial_balances": [{"address":"'"$1"'","amount":"1000000000"},{"address":"'"$($BINARY keys show validator -a)"'","amount":"1000000000"}]
}'
echo "$CW20_INIT"
echo xxxxxxxxx | $BINARY tx wasm instantiate $CW20_CODE "$CW20_INIT" --from "validator" --label "token" $TXFLAG

# Get cw20 contract address
CW20_CONTRACT=$($BINARY q wasm list-contract-by-code $CW20_CODE --output json $QFLAG | jq -r '.contracts[-1]')
echo $CW20_CONTRACT

# Upload cw-dao contract code
echo xxxxxxxxx | $BINARY tx wasm store "/wasmswap.wasm" --from validator $TXFLAG
WASMSWAP_CODE=$($BINARY q wasm list-code --reverse --output json $QFLAG | jq -r '.code_infos[0].code_id')
echo $WASMSWAP_CODE

# Upload staking contract code
echo xxxxxxxxx | $BINARY tx wasm store "/stake_cw20.wasm" --from validator $TXFLAG
STAKING_CODE=$($BINARY q wasm list-code --reverse --output json $QFLAG | jq -r '.code_infos[0].code_id')

echo $STAKING_CODE

# Upload staking rewards contract code
echo xxxxxxxxx | $BINARY tx wasm store "/stake_cw20_external_rewards.wasm" --from validator $TXFLAG
STAKING_REWARDS_CODE=$($BINARY q wasm list-code --reverse --output json $QFLAG | jq -r '.code_infos[0].code_id')

echo $STAKING_REWARDS_CODE


echo $CW20_CODE
echo $WASMSWAP_CODE
echo $STAKING_CODE
echo $STAKING_REWARDS_CODE

# Initialize factory contract
SWAP_1_INIT='{
    "token1_denom": {"native": "'$DENOM'"},
    "token2_denom": {"cw20": "'"$CW20_CONTRACT"'"},
    "lp_token_code_id": '$CW20_CODE',
    "owner": "'$1'",
    "protocol_fee_recipient": "'$1'",
    "protocol_fee_percent": "0.2",
    "lp_fee_percent": "0.3"
}'

echo "$SWAP_1_INIT"
echo xxxxxxxxx | $BINARY tx wasm instantiate $WASMSWAP_CODE "$SWAP_1_INIT" --from "validator" --label "swap_1" $TXFLAG
SWAP_1_CONTRACT=$($BINARY q wasm list-contract-by-code $WASMSWAP_CODE --output json $QFLAG | jq -r '.contracts[-1]')
echo $SWAP_1_CONTRACT
$BINARY tx wasm execute $CW20_CONTRACT '{"increase_allowance":{"amount":"100000000","spender":"'"$SWAP_1_CONTRACT"'"}}' --from test $TXFLAG
$BINARY tx wasm execute $SWAP_1_CONTRACT '{"add_liquidity":{"token1_amount":"100000000","max_token2":"100000000","min_liquidity":"1"}}' --from test --amount "100000000"$DENOM $TXFLAG

# Instantiate cw20 contract
CW20_INIT_2='{
    "name": "DAO Coin",
    "symbol": "DAO",
    "decimals": 6,
    "initial_balances": [{"address":"'"$1"'","amount":"1000000000"}]
}'
echo "$CW20_INIT_2"
echo xxxxxxxxx | $BINARY tx wasm instantiate $CW20_CODE "$CW20_INIT_2" --from "validator" --label "token" $TXFLAG

# Get cw20 contract address
CW20_CONTRACT_2=$($BINARY q wasm list-contract-by-code $CW20_CODE --output json $QFLAG | jq -r '.contracts[-1]')
echo $CW20_CONTRACT_2

# Initialize factory contract
SWAP_2_INIT='{
    "token1_denom": {"native": "'$DENOM'"},
    "token2_denom": {"cw20": "'"$CW20_CONTRACT_2"'"},
    "lp_token_code_id": '$CW20_CODE'
    "owner": "'$1'",
    "protocol_fee_recipient": "'$1'",
    "protocol_fee_percent": "0.2",
    "lp_fee_percent": "0.3"
}'

echo "$SWAP_2_INIT"
echo xxxxxxxxx | $BINARY tx wasm instantiate $WASMSWAP_CODE "$SWAP_2_INIT" --from "validator" --label "swap_2" $TXFLAG
SWAP_2_CONTRACT=$($BINARY q wasm list-contract-by-code $WASMSWAP_CODE --output json $QFLAG | jq -r '.contracts[-1]')
$BINARY tx wasm execute $CW20_CONTRACT_2 '{"increase_allowance":{"amount":"100000000","spender":"'"$SWAP_2_CONTRACT"'"}}' --from test $TXFLAG
$BINARY tx wasm execute $SWAP_2_CONTRACT '{"add_liquidity":{"token1_amount":"100000000","max_token2":"100000000","min_liquidity":"1"}}' --from test --amount "100000000"$DENOM $TXFLAG

# Instantiate cw20 contract
CW20_INIT_3='{
    "name": "POOD Coin",
    "symbol": "POOD",
    "decimals": 6,
    "initial_balances": [{"address":"'"$1"'","amount":"1000000000"}]
}'
echo "$CW20_INIT_3"
echo xxxxxxxxx | $BINARY tx wasm instantiate $CW20_CODE "$CW20_INIT_3" --from "validator" --label "token" $TXFLAG

# Get cw20 contract address
CW20_CONTRACT_3=$($BINARY q wasm list-contract-by-code $CW20_CODE --output json $QFLAG | jq -r '.contracts[-1]')
echo $CW20_CONTRACT_3

# Initialize factory contract
SWAP_3_INIT='{
    "token1_denom": {"native": "'$DENOM'"},
    "token2_denom": {"cw20": "'"$CW20_CONTRACT_3"'"},
    "lp_token_code_id": '$CW20_CODE',
    "owner": "'$1'",
    "protocol_fee_recipient": "'$1'",
    "protocol_fee_percent": "0.2",
    "lp_fee_percent": "0.3"
}'

echo "$SWAP_3_INIT"
echo xxxxxxxxx | $BINARY tx wasm instantiate $WASMSWAP_CODE "$SWAP_3_INIT" --from "validator" --label "swap_3" $TXFLAG
SWAP_3_CONTRACT=$($BINARY q wasm list-contract-by-code $WASMSWAP_CODE --output json $QFLAG | jq -r '.contracts[-1]')

$BINARY tx wasm execute $CW20_CONTRACT_3 '{"increase_allowance":{"amount":"100000000","spender":"'"$SWAP_3_CONTRACT"'"}}' --from test $TXFLAG
$BINARY tx wasm execute $SWAP_3_CONTRACT '{"add_liquidity":{"token1_amount":"100000000","max_token2":"100000000","min_liquidity":"1"}}' --from test --amount "100000000"$DENOM $TXFLAG


# Initialize factory contract
SWAP_4_INIT='{
    "token1_denom": {"native": "'$DENOM'"},
    "token2_denom": {"native": "ucosm"},
    "lp_token_code_id": '$CW20_CODE',
    "owner": "'$1'",
    "protocol_fee_recipient": "'$1'",
    "protocol_fee_percent": "0.2",
    "lp_fee_percent": "0.3"
}'

echo "$SWAP_4_INIT"
echo xxxxxxxxx | $BINARY tx wasm instantiate $WASMSWAP_CODE "$SWAP_4_INIT" --from "validator" --label "swap_4" $TXFLAG
SWAP_4_CONTRACT=$($BINARY q wasm list-contract-by-code $WASMSWAP_CODE --output json $QFLAG | jq -r '.contracts[-1]')

$BINARY tx wasm execute $SWAP_4_CONTRACT '{"add_liquidity":{"token1_amount":"100000000","max_token2":"100000000","min_liquidity":"1"}}' --from test --amount "100000000"$DENOM",100000000ucosm" $TXFLAG

SWAP_1_TOKEN_ADDRESS=$($BINARY query wasm contract-state smart $SWAP_1_CONTRACT '{"info":{}}' --output json $QFLAG | jq -r '.data.lp_token_address')
echo $SWAP_1_TOKEN_ADDRESS

# Swap 5
SWAP_5_INIT='{
    "token1_denom": {"cw20": "'$CW20_CONTRACT'"},
    "token2_denom": {"cw20": "'"$CW20_CONTRACT_2"'"},
    "lp_token_code_id": '$CW20_CODE'
    "owner": "'$1'",
    "protocol_fee_recipient": "'$1'",
    "protocol_fee_percent": "0.2",
    "lp_fee_percent": "0.3"
}'

echo "$SWAP_5_INIT"
echo xxxxxxxxx | $BINARY tx wasm instantiate $WASMSWAP_CODE "$SWAP_5_INIT" --from "validator" --label "swap_5" $TXFLAG
SWAP_5_CONTRACT=$($BINARY q wasm list-contract-by-code $WASMSWAP_CODE --output json $QFLAG | jq -r '.contracts[-1]')

$BINARY tx wasm execute $CW20_CONTRACT '{"increase_allowance":{"amount":"100000000","spender":"'"$SWAP_5_CONTRACT"'"}}' --from test $TXFLAG
$BINARY tx wasm execute $CW20_CONTRACT_2 '{"increase_allowance":{"amount":"100000000","spender":"'"$SWAP_5_CONTRACT"'"}}' --from test $TXFLAG
$BINARY tx wasm execute $SWAP_5_CONTRACT '{"add_liquidity":{"token1_amount":"100000000","max_token2":"100000000","min_liquidity":"1"}}' --from test --amount "100000000"$DENOM $TXFLAG

echo $SWAP_5_CONTRACT

# Instantiate staking contract
STAKING_1_INIT='{
    "owner": "'"$($BINARY keys show validator -a)"'",
    "token_address": "'"$SWAP_1_TOKEN_ADDRESS"'",
    "unstaking_duration": {"time":30}
}'
echo $STAKING_1_INIT
echo xxxxxxxxx | $BINARY tx wasm instantiate $STAKING_CODE "$STAKING_1_INIT" --from "validator" --label "staking_1" $TXFLAG
STAKING_1_CONTRACT=$($BINARY q wasm list-contract-by-code $STAKING_CODE --output json $QFLAG | jq -r '.contracts[-1]')


SWAP_2_TOKEN_ADDRESS=$($BINARY query wasm contract-state smart $SWAP_2_CONTRACT '{"info":{}}' --output json $QFLAG | jq -r '.data.lp_token_address')
echo $SWAP_2_TOKEN_ADDRESS

# Instantiate staking contract
STAKING_2_INIT='{
    "owner": "'"$($BINARY keys show validator -a)"'",
    "token_address": "'"$SWAP_2_TOKEN_ADDRESS"'",
    "unstaking_duration": {"time":30}
}'
echo $STAKING_2_INIT
echo xxxxxxxxx | $BINARY tx wasm instantiate $STAKING_CODE "$STAKING_2_INIT" --from "validator" --label "staking_1" $TXFLAG
STAKING_2_CONTRACT=$($BINARY q wasm list-contract-by-code $STAKING_CODE --output json $QFLAG | jq -r '.contracts[-1]')


SWAP_3_TOKEN_ADDRESS=$($BINARY query wasm contract-state smart $SWAP_3_CONTRACT '{"info":{}}' --output json $QFLAG | jq -r '.data.lp_token_address')
echo $SWAP_3_TOKEN_ADDRESS

# Instantiate staking contract
STAKING_3_INIT='{
    "owner": "'"$($BINARY keys show validator -a)"'",
    "token_address": "'"$SWAP_3_TOKEN_ADDRESS"'",
    "unstaking_duration": {"time":30}
}'
echo $STAKING_3_INIT
echo xxxxxxxxx | $BINARY tx wasm instantiate $STAKING_CODE "$STAKING_3_INIT" --from "validator" --label "staking_1" $TXFLAG
STAKING_3_CONTRACT=$($BINARY q wasm list-contract-by-code $STAKING_CODE --output json $QFLAG | jq -r '.contracts[-1]')


SWAP_4_TOKEN_ADDRESS=$($BINARY query wasm contract-state smart $SWAP_4_CONTRACT '{"info":{}}' --output json $QFLAG | jq -r '.data.lp_token_address')
echo $SWAP_4_TOKEN_ADDRESS

# Instantiate staking contract
STAKING_4_INIT='{
    "owner": "'"$($BINARY keys show validator -a)"'",
    "token_address": "'"$SWAP_4_TOKEN_ADDRESS"'",
    "unstaking_duration": {"time":30}
}'
echo $STAKING_4_INIT
echo xxxxxxxxx | $BINARY tx wasm instantiate $STAKING_CODE "$STAKING_4_INIT" --from "validator" --label "staking_1" $TXFLAG
STAKING_4_CONTRACT=$($BINARY q wasm list-contract-by-code $STAKING_CODE --output json $QFLAG | jq -r '.contracts[-1]')

# Instantiate reward contracts
REWARDS_1_1_INIT='{
      "owner": "'"$($BINARY keys show validator -a)"'",
      "manager": "'"$($BINARY keys show validator -a)"'",
      "staking_contract":"'"$STAKING_1_CONTRACT"'",
      "reward_token": {"native": "'$DENOM'"},
      "reward_duration": 100000
}'

echo $REWARDS_1_1_INIT
echo $STAKING_REWARDS_CODE
echo xxxxxxxxx | $BINARY tx wasm instantiate $STAKING_REWARDS_CODE "$REWARDS_1_1_INIT" --from "validator" --label "rewards_1" $TXFLAG
REWARDS_1_1_CONTRACT=$($BINARY q wasm list-contract-by-code $STAKING_REWARDS_CODE --output json $QFLAG | jq -r '.contracts[-1]')

$BINARY tx wasm execute $STAKING_1_CONTRACT '{"add_hook":{"addr":"'$REWARDS_1_1_CONTRACT'"}}' --from validator $TXFLAG

$BINARY tx wasm execute $REWARDS_1_1_CONTRACT '{"fund":{}}' --from validator --amount "100000000"$DENOM $TXFLAG

# Instantiate reward contracts
REWARDS_1_2_INIT='{
      "owner": "'"$($BINARY keys show validator -a)"'",
      "staking_contract":"'"$STAKING_1_CONTRACT"'",
      "reward_token": {"cw20": "'"$CW20_CONTRACT"'"},
      "reward_duration": 100000
}'

echo $REWARDS_1_2_INIT
echo $STAKING_REWARDS_CODE
echo xxxxxxxxx | $BINARY tx wasm instantiate $STAKING_REWARDS_CODE "$REWARDS_1_2_INIT" --from "validator" --label "rewards_1" $TXFLAG
REWARDS_1_2_CONTRACT=$($BINARY q wasm list-contract-by-code $STAKING_REWARDS_CODE --output json $QFLAG | jq -r '.contracts[-1]')

$BINARY tx wasm execute $STAKING_1_CONTRACT '{"add_hook":{"addr":"'$REWARDS_1_2_CONTRACT'"}}' --from validator $TXFLAG

REWARD_2_FUND_SUB_MSG='{"fund":{}}'
REWARD_2_FUND_SUB_MSG_BINARY="$(echo $REWARD_2_FUND_SUB_MSG | base64)"
echo $REWARD_2_FUND_SUB_MSG
echo $REWARD_2_FUND_SUB_MSG_BINARY

REWARD_2_FUND_MSG='{
 "send":{
  "contract": "'"$REWARDS_1_2_CONTRACT"'",
  "amount":"100000000",
  "msg": "'"$REWARD_2_FUND_SUB_MSG_BINARY"'"
 }
}'
echo $REWARD_2_FUND_MSG
$BINARY tx wasm execute $CW20_CONTRACT "$REWARD_2_FUND_MSG" --from validator $TXFLAG


echo "CRAB cw20 contract 1"
echo $CW20_CONTRACT
echo "CRAB Swap contract 1"
echo $SWAP_1_CONTRACT
echo "CRAB Staking contract 1"
echo $STAKING_1_CONTRACT
echo "CRAB Rewards contract 1"
echo $REWARDS_1_1_CONTRACT
echo "CRAB Rewards contract 2"
echo $REWARDS_1_2_CONTRACT
echo "DAO cw20 contract 2"
echo $CW20_CONTRACT_2
echo "DAO Swap contract 2"
echo $SWAP_2_CONTRACT
echo "DAO Staking contract 1"
echo $STAKING_2_CONTRACT
echo "POOD cw20 contract 3"
echo $CW20_CONTRACT_3
echo "POOD Swap contract 3"
echo $SWAP_3_CONTRACT
echo "POOD Staking contract 1"
echo $STAKING_3_CONTRACT
echo "COSM SWap contract 4"
echo $SWAP_4_CONTRACT
echo "POOD Staking contract 1"
echo $STAKING_4_CONTRACT
echo "CRAB <> DAO Swap contract 5"
echo $SWAP_5_CONTRACT

