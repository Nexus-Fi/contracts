# Contract optiimization
# sudo docker run --rm -v "$(pwd)":/code --mount type=volume,source="$(basename "$(pwd)")_cache",target=/target --mount type=volume,source=registry_cache,target=/usr/local/cargo/registry cosmwasm/optimizer:0.15.0
# sudo docker run --rm -v "$(pwd)/contracts":/code --mount type=volume,source="$(basename "$(pwd)")_cache",target=/target --mount type=volume,source=registry_cache,target=/usr/local/cargo/registry cosmwasm/optimizer:0.15.0
# sudo docker run --rm -v "$(pwd)":/code \
#     --mount type=volume,source="$(basename "$(pwd)")_cache",target=/code/target \
#     --mount type=volume,source=registry_cache,target=/usr/local/cargo/registry \
#     cosmwasm/$image:$image_version
# chain config
nibid config chain-id nibiru-testnet-1 &&                                      
nibid config broadcast-mode sync && 
nibid config node "https://rpc.testnet-1.nibiru.fi:443" && 
nibid config keyring-backend os && 
nibid config output json


FROM=nibi1hzty850q3vnew33yuft82j0v5fazyvfcescxhs


TXHASH="$(nibid tx wasm store artifacts/nexus_rewards_dispatcher.wasm \
    --from $FROM \
    --gas auto \
    --gas-adjustment 1.5 \
    --gas-prices 0.025unibi \
    --yes | jq -rcs '.[0].txhash')"
echo 'TXHASH:'
echo $TXHASH
sleep 6
nibid q tx $TXHASH >nexus_rewards_dispatcher.json
CODE_ID="$(cat nexus_rewards_dispatcher.json | jq -r '.logs[0].events[1].attributes[1].value')"
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
    "$(cat initiate_dispatcher.json)" \
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
nibid q tx $TXHASH_INIT >initiate_dispatcher.init.json

CONTRACT_ADDRESS="$(cat initiate_dispatcher.init.json | jq -r '.logs[0].events[1].attributes[0].value')"
echo 'CONTRACT_ADDRESS:'
echo $CONTRACT_ADDRESS

# TXHASH:
# 4689E1C6E26CC061545CE35AC091017D9E2E8D84D3B087B5094A836B82196587
# CODE_ID:
# 884
# gas estimate: 222319
# TXHASH_INIT:
# 2D001180879539BFAB7768E9C369B75EF0AC5DFC036944735212B233CAEE52A4
# CONTRACT_ADDRESS:
# nibi1vztltq8jw5n3khcrdkewhpq07alh0m8l4r4e4c5e8tluv7vm207qt5f2gp

# nibid tx wasm execute $CONTRACT_ADDRESS "$(cat dispatch_rewards.json)" \
#       --from $FROM \
#       --gas auto \
#       --gas-adjustment 1.5 \
#       --gas-prices 0.025unibi \
#       --yes | jq -rcs '.[0].txhash'



# nibid tx wasm execute $CONTRACT_ADDRESS "$(cat execute_bond_staking.json)" \ 
#       --amount 3000000unibi \
#       --from $FROM \         
#       --gas auto \          
#       --gas-adjustment 1.5 \   
#       --gas-prices 0.025unibi \    
#       --yes | jq -rcs '.[0].txhash'

