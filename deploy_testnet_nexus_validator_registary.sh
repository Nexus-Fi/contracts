# sudo docker run --rm -v "$(pwd)":/code --mount type=volume,source="$(basename "$(pwd)")_cache",target=/target --mount type=volume,source=registry_cache,target=/usr/local/cargo/registry cosmwasm/optimizer:0.15.0

# chain config
nibid config chain-id nibiru-testnet-1 &&                                      
nibid config broadcast-mode sync && 
nibid config node "https://rpc.testnet-1.nibiru.fi:443" && 
nibid config keyring-backend os && 
nibid config output json


FROM=nibi1hzty850q3vnew33yuft82j0v5fazyvfcescxhs


TXHASH="$(nibid tx wasm store artifacts/nexus_validator_registary.wasm \
    --from $FROM \
    --gas auto \
    --gas-adjustment 1.5 \
    --gas-prices 0.025unibi \
    --yes | jq -rcs '.[0].txhash')"
echo 'TXHASH:'
echo $TXHASH
sleep 6
nibid q tx $TXHASH >nexus_validator_registary.json
CODE_ID="$(cat nexus_validator_registary.json | jq -r '.logs[0].events[1].attributes[1].value')"
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
    "$(cat initiate_registary.json)" \
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
nibid q tx $TXHASH_INIT >initiate_registary.init.json

CONTRACT_ADDRESS="$(cat initiate_registary.init.json | jq -r '.logs[0].events[1].attributes[0].value')"
echo 'CONTRACT_ADDRESS:'
echo $CONTRACT_ADDRESS



# nibid tx wasm execute $CONTRACT_ADDRESS "$(cat update_config_validator_registary.json)" \
#       --from $FROM \
#       --gas auto \
#       --gas-adjustment 1.5 \
#       --gas-prices 0.025unibi \
#       --yes | jq -rcs '.[0].txhash'