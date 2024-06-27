# Contract optiimization
# sudo docker run --rm -v "$(pwd)":/code --mount type=volume,source="$(basename "$(pwd)")_cache",target=/target --mount type=volume,source=registry_cache,target=/usr/local/cargo/registry cosmwasm/optimizer:0.16.0
# sudo docker run --rm -v "$(pwd)/contracts":/code --mount type=volume,source="$(basename "$(pwd)")_cache",target=/target --mount type=volume,source=registry_cache,target=/usr/local/cargo/registry cosmwasm/optimizer:0.16.0
# sudo docker run --rm -v "$(pwd)":/code \
#     --mount type=volume,source="$(basename "$(pwd)")_cache",target=/code/target \
#     --mount type=volume,source=registry_cache,target=/usr/local/cargo/registry \
#     cosmwasm/$image:$image_version
# chain config
# sudo docker run --rm -v "$(pwd)":/code \
#   --mount type=volume,source="$(basename "$(pwd)")_cache",target=/target \
#   --mount type=volume,source=registry_cache,target=/usr/local/cargo/registry \
#   cosmwasm/optimizer:0.16.0

nibid config chain-id nibiru-testnet-1 &&                                      
nibid config broadcast-mode sync && 
nibid config node "https://rpc.testnet-1.nibiru.fi:443" && 
nibid config keyring-backend os && 
nibid config output json


FROM=nibi1hzty850q3vnew33yuft82j0v5fazyvfcescxhs

      
TXHASH="$(nibid tx wasm store artifacts/nexus_staking_nibi.wasm \
    --from $FROM \
    --gas auto \
    --gas-adjustment 1.5 \
    --gas-prices 0.025unibi \
    --yes | jq -rcs '.[0].txhash' )"
echo 'TXHASH:'
echo $TXHASH
sleep 6
nibid q tx $TXHASH >nexus_staking_nibi.json
CODE_ID="$(cat nexus_staking_nibi.json | jq -r '.logs[0].events[1].attributes[1].value')"
echo 'CODE_ID:'
echo $CODE_ID

# echo "{
#   "initial_owner": "nibi1hzty850q3vnew33yuft82j0v5fazyvfcescxhs",
#   "min_withdrawal_delay_blocks": 1000,
#   "strategies": [
#     "nibi1hzty850q3vnew33yuft82j0v5fazyvfcescxhs"
#   ],
#   "withdrawal_delay_blocks": [
#     10,
#     20,
#     30
#   ]
# }
# " | jq . | tee delgationmanager_instantiate.json

sleep 10

TXHASH_INIT="$(nibid tx wasm instantiate $CODE_ID \
    "$(cat inititate_staking.json)" \
    --admin "$FROM" \
    --label "contract" \
    --from $FROM \
    --gas auto \
    --gas-adjustment 1.5 \
    --gas-prices 0.025unibi \
    --yes | jq -rcs '.[0].txhash')"

echo 'TXHASH_INIT:'
echo $TXHASH_INIT
sleep 6
nibid q tx $TXHASH_INIT >inititate_staking.init.json

CONTRACT_ADDRESS="$(cat inititate_staking.init.json | jq -r '.logs[0].events[1].attributes[0].value')"
echo 'CONTRACT_ADDRESS:'
echo $CONTRACT_ADDRESS


# nibid tx wasm execute $CONTRACT_ADDRESS "$(cat recieve.json)" \
#       --amount 3tf/nibi184n3nnvufu7gqch7md3ne6qyy4lmes5hmqfzd3ehm7atcm2gxwtqmx5z6a/zzz \
#       --from $FROM \
#       --gas auto \
#       --gas-adjustment 1.5 \
#       --gas-prices 0.025unibi \
#       --yes | jq -rcs '.[0].txhash'

nibid tx wasm execute $CONTRACT_ADDRESS "$(cat update_config_staking.json)" \
      --from $FROM \
      --gas auto \
      --gas-adjustment 1.5 \
      --gas-prices 0.025unibi \
      --yes | jq -rcs '.[0].txhash'

    #   nibid tx wasm execute $CONTRACT_ADDRESS "$(cat withdraw_unbonded.json)" \
    #   --from $FROM \
    #   --gas auto \
    #   --gas-adjustment 1.5 \
    #   --gas-prices 0.025unibi \
    #   --yes | jq -rcs '.[0].txhash'

# nibid tx wasm execute $CONTRACT_ADDRESS "$(cat execute_bond_staking.json)" \
#       --amount 3000000unibi \
#       --from $FROM \
#       --gas auto \
#       --gas-adjustment 1.5 \
#       --gas-prices 0.025unibi \
#       --yes | jq -rcs '.[0].txhash'

# nibid tx wasm execute $CONTRACT_ADDRESS "$(cat restake.json)" \
#       --amount 1stakedNIBI \
#       --from $FROM \
#       --gas auto \
#       --gas-adjustment 1.5 \
#       --gas-prices 0.025unibi \
#       --yes | jq -rcs '.[0].txhash'