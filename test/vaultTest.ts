
import {
    NibiruTxClient,
    newSignerFromMnemonic,
    Testnet,
    NibiruQuerier,
  } from "@nibiruchain/nibijs"
import { coins } from "@cosmjs/proto-signing"
const chain = Testnet(1);
const RPC_ENDPOINT = "https://rpc.nibiru.fi:443";
const CHAIN_ID = "nibiru-testnet-1";
const STNIBI_DENOM = "tf/nibi1xyaaw84yafry7afw00sedvzkl306tydkcgc6f6wpjj2z5yx86agsddm72f/newt";
const VAULT_CONTRACT_ADDRESS = "nibi175axfpu4a5ayfnj3nrj498ygqp9x3q066p9cpdcjjsqm596zcrqqzrtrzq";
export type JsonObject = any;
export interface TokenToSend {
  denom: string;
  amount: string;
}
import { Coin } from "@cosmjs/amino";
export const CHAIN = Testnet(1)
async function main() {

const mnemonic = "chef doll jump dwarf debate pottery cactus robot sustain summer thunder heavy refuse area town noble enter ridge tomato nasty mesh knock divorce tuna"
const lockMsg = {
        lock: {}
      };
    
const signer = await newSignerFromMnemonic(mnemonic)
const querier = await NibiruQuerier.connect(CHAIN.endptTm)
const txClient = await NibiruTxClient.connectWithSigner(CHAIN.endptTm, signer)
const [{ address: fromAddr }] = await signer.getAccounts()
const tokens = coins(5, STNIBI_DENOM)


const coin: Coin = {
    denom: STNIBI_DENOM,
    amount: "1000000"
  };
const signingClient = await NibiruTxClient.connectWithSigner(
    chain.endptTm,
    signer!
  );
  const senderAddress = "nibi150we6pt79u2q5sd38j3lceu6mwlfww08dvp570"; 
  try {
    const result = await signingClient.wasmClient.execute(
      senderAddress,
      VAULT_CONTRACT_ADDRESS,
      lockMsg,
      "auto",
      "Locking stNIBI",
      [coin]
    );

    console.log("Transaction hash:", result.transactionHash);
    return result.transactionHash;
  } catch (error) {
    console.error("Error locking stNIBI:", error);
    throw error;
  }

  }
  
  main().catch(console.error);
