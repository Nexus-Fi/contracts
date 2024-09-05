import { SigningStargateClient, StdFee, GasPrice } from "@cosmjs/stargate";
import { DirectSecp256k1HdWallet } from "@cosmjs/proto-signing";
import { fromUtf8, toUtf8 } from "@cosmjs/encoding";
import { ethers } from "ethers";
import WebSocket from "ws";
import dotenv from 'dotenv';
dotenv.config();
// Cosmos chain configuration
const RPC_ENDPOINT = "https://rpc.nibiru.fi:443";
const CHAIN_ID = "nibiru-testnet-1";
const STNIBI_DENOM = "tf/nibi1xyaaw84yafry7afw00sedvzkl306tydkcgc6f6wpjj2z5yx86agsddm72f/newt";
const VAULT_CONTRACT_ADDRESS = "nibi175axfpu4a5ayfnj3nrj498ygqp9x3q066p9cpdcjjsqm596zcrqqzrtrzq";
const TEST_ACCOUNT_ADDRESS = "0x5B38Da6a701c568545dCfcB03FcB875f56beddC4";
// EVM configuration
const EVM_RPC_ENDPOINT = "https://evm-rpc.nibiru.fi";
const EVM_BRIDGE_ADDRESS = "0x5FbDB2315678afecb367f032d93F642f64180aa3";
const EVM_BRIDGE_ABI = [
 // Function to mint stNIBI tokens on the EVM side
 "function bridgeToEVM(address to, uint256 amount) external",
  
 // Function to burn stNIBI tokens when bridging back to Cosmos
 "function bridgeFromEVM(address from, uint256 amount) external",
 
 // Event emitted when tokens are bridged to EVM
 "event BridgedToEVM(address indexed to, uint256 amount)",
 
 // Event emitted when tokens are bridged from EVM back to Cosmos
 "event BridgedFromEVM(address indexed from, uint256 amount)",
 
 // Optional: Function to pause the bridge in case of emergencies
 "function pauseBridge() external",
 
 // Optional: Function to unpause the bridge
 "function unpauseBridge() external",
 
 // Optional: Function to check if the bridge is paused
 "function isPaused() external view returns (bool)",
 
 // Optional: Function to get the total amount of stNIBI minted on EVM
 "function totalSupply() external view returns (uint256)"
];

// Function to get Cosmos wallet
async function getCosmosWallet(mnemonic: string): Promise<DirectSecp256k1HdWallet> {
  return await DirectSecp256k1HdWallet.fromMnemonic(mnemonic, { prefix: "nibi" });
}

// Function to get Cosmos signing client
async function getCosmosSigningClient(wallet: DirectSecp256k1HdWallet): Promise<SigningStargateClient> {
  return await SigningStargateClient.connectWithSigner(RPC_ENDPOINT, wallet);
}


type EvmWalletType = ethers.Wallet | ethers.JsonRpcSigner;

async function getEvmWallet(privateKeyOrAddress: string): Promise<EvmWalletType> {
  if (process.env.NODE_ENV === 'development') {
    const provider = new ethers.JsonRpcProvider('http://localhost:8545');
    return await provider.getSigner(privateKeyOrAddress);
  } else {
    return new ethers.Wallet(privateKeyOrAddress, new ethers.JsonRpcProvider(EVM_RPC_ENDPOINT));
  }
}

async function mintStNibiOnEvm(evmWallet: EvmWalletType, to: string, amount: string) {
  const bridgeContract = new ethers.Contract(EVM_BRIDGE_ADDRESS, EVM_BRIDGE_ABI, evmWallet);
  
  try {
    const tx = await bridgeContract.bridgeToEVM(to, amount);
    await tx.wait();
    console.log(`Minted ${amount} stNIBI on EVM for address ${to}. Transaction hash: ${tx.hash}`);
  } catch (error) {
    console.error("Error minting stNIBI on EVM:", error);
  }
}

async function monitorCosmosChain(cosmosWallet: DirectSecp256k1HdWallet, evmWallet: EvmWalletType) {
  const wsUrl = RPC_ENDPOINT.replace("http", "ws");
  const ws = new WebSocket(`${wsUrl}/websocket`);

  ws.on("open", () => {
    console.log("Connected to Nibiru WebSocket");
    const subscriptionMsg = {
      jsonrpc: "2.0",
      method: "subscribe",
      id: "1",
      params: {
        query: `tm.event='Tx' AND wasm._contract_address='${VAULT_CONTRACT_ADDRESS}' AND wasm.action='lock'`
      }
    };
    ws.send(JSON.stringify(subscriptionMsg));
  });

  ws.on("message", async (data: WebSocket.Data) => {
    const response = JSON.parse(data.toString());
    if (response.result && response.result.data && response.result.data.value && response.result.data.value.TxResult) {
      const txResult = response.result.data.value.TxResult;
      const events = txResult.result.events;

      for (const event of events) {
        if (event.type === "wasm" && event.attributes) {
          const lockEvent = event.attributes.find((attr: any) => 
            fromUtf8(attr.key) === "action" && fromUtf8(attr.value) === "lock"
          );

          if (lockEvent) {
            const userAttr = event.attributes.find((attr: any) => fromUtf8(attr.key) === "user");
            const amountAttr = event.attributes.find((attr: any) => fromUtf8(attr.key) === "amount");

            if (userAttr && amountAttr) {
              const user = fromUtf8(userAttr.value);
              const amount = fromUtf8(amountAttr.value);
              console.log(`Lock event detected: User ${user} locked ${amount} stNIBI`);
              
              await mintStNibiOnEvm(evmWallet, user, amount);
            }
          }
        }
      }
    }
  });

  ws.on("error", (error: Error) => {
    console.error("WebSocket error:", error);
  });

  ws.on("close", () => {
    console.log("Disconnected from Nibiru WebSocket");
  });
}

async function main() {
  const cosmosMnemonic = process.env.COSMOS_MNEMONIC;
  const evmPrivateKeyOrAddress = process.env.NODE_ENV === 'development' ? TEST_ACCOUNT_ADDRESS : process.env.EVM_PRIVATE_KEY;

  if (!cosmosMnemonic || !evmPrivateKeyOrAddress) {
    throw new Error("Missing environment variables. Please check your .env file.");
  }

  const cosmosWallet = await DirectSecp256k1HdWallet.fromMnemonic(cosmosMnemonic, { prefix: "nibi" });
  const evmWallet = await getEvmWallet(evmPrivateKeyOrAddress);

  await monitorCosmosChain(cosmosWallet, evmWallet);
}

main().catch(console.error);