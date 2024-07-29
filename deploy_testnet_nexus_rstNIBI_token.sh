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


TXHASH="$(nibid tx wasm store artifacts/nexus_restake_rstnibi.wasm \
    --from $FROM \
    --gas auto \
    --gas-adjustment 1.5 \
    --gas-prices 0.025unibi \
    --yes | jq -rcs '.[0].txhash')"
echo 'TXHASH:'
echo $TXHASH
sleep 6
nibid q tx $TXHASH >nexus_rstnibi_token_test.json
CODE_ID="$(cat nexus_rstnibi_token_test.json | jq -r '.logs[0].events[1].attributes[1].value')"
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

# sleep 10

TXHASH_INIT="$(nibid tx wasm instantiate $CODE_ID \
    "$(cat initiate_token_rstnibi.json)" \
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
nibid q tx $TXHASH_INIT >initiate_token_rstnibi.init.json

CONTRACT_ADDRESS="$(cat initiate_token_rstnibi.init.json | jq -r '.logs[0].events[1].attributes[0].value')"
echo 'CONTRACT_ADDRESS:'
echo $CONTRACT_ADDRESS


# TXHASH:
# BA56A11D8B39AE44A948837FF5A5DCC59A24EB05A2B49FB08CDE9CD8D51A157B
# CODE_ID:
# 887
# gas estimate: 262927
# TXHASH_INIT:
# 21261046FED586C9486D286D382FFD566613304AA6CEA46162DF360C812915FA
# CONTRACT_ADDRESS:
# nibi1a9huv32h6s6x906z6dl58h2dd9trc2sn7945pgreqjxjkg5ukpdsln0sdl

# nibid tx wasm execute $CONTRACT_ADDRESS "$(cat send_from.json)" \
#       --from $FROM \
#       --gas auto \
#       --gas-adjustment 1.5 \
#       --gas-prices 0.025unibi \
#       --yes | jq -rcs '.[0].txhash'

#       nibid tx wasm execute $CONTRACT_ADDRESS "$(cat update_config_staking.json)" \
#       --from $FROM \
#       --gas auto \
#       --gas-adjustment 1.5 \
#       --gas-prices 0.025unibi \
#       --yes | jq -rcs '.[0].txhash'

# nibid tx wasm execute $CONTRACT_ADDRESS "$(cat withdraw_liquidity.json)" \
#       --from $FROM \
#       --gas auto \
#       --gas-adjustment 1.5 \
#       --gas-prices 0.025unibi \
#       --yes | jq -rcs '.[0].txhash'
      

#       nibid tx wasm execute $CONTRACT_ADDRESS "$(cat execute_bond_staking.json)" \
#       --from $FROM \
#       --gas auto \
#       --gas-adjustment 1.5 \
#       --gas-prices 0.025unibi \
#       --yes | jq -rcs '.[0].txhash'

# nibid tx wasm execute $CONTRACT_ADDRESS "$(cat restake.json)" \
#       --amount 1000000nsunibi \
#       --from $FROM \
#       --gas auto \
#       --gas-adjustment 1.5 \
#       --gas-prices 0.025unibi \
#       --yes | jq -rcs '.[0].txhash'

#       https://github.com/Nexus-Fi/NexusFi-contracts/issues/2#issuecomment-2166465918


#       nibid tx wasm execute $CONTRACT_ADDRESS "$(cat update_logo.json)" \
#       --from $FROM \
#       --gas auto \
#       --gas-adjustment 1.5 \
#       --gas-prices 0.025unibi \
#       --yes | jq -rcs '.[0].txhash'