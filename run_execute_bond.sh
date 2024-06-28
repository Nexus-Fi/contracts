nibid config chain-id nibiru-testnet-1 &&                                      
nibid config broadcast-mode sync && 
nibid config node "https://rpc.testnet-1.nibiru.fi:443" && 
nibid config keyring-backend os && 
nibid config output json


FROM=nibi1hzty850q3vnew33yuft82j0v5fazyvfcescxhs
CONTRACT_ADDRESS=nibi1nkq0jz25qwmajsrc2460zjjfktkf38zwh08lzrt76hxhd0d0z5zq3ylthd

nibid tx wasm execute $CONTRACT_ADDRESS "$(cat execute_bond_staking.json)" \
      --amount 3000000unibi \
      --from $FROM \
      --gas auto \
      --gas-adjustment 1.5 \
      --gas-prices 0.025unibi \
      --yes | jq -rcs '.[0].txhash'