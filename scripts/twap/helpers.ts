import * as fs from "fs";
import BN from "bn.js";

import { SigningCosmWasmClient } from "@cosmjs/cosmwasm-stargate";

/**
 * @notice Encode a JSON object to base64 binary
 */
export function toEncodedBinary(obj: any) {
  return Buffer.from(JSON.stringify(obj)).toString("base64");
}

export function toEncodedBinaryUint8(obj: any) {
  return Buffer.from(JSON.stringify(obj));
}

/**
 * @notice Upload contract code to LocalTerra. Return code ID.
 */
export async function storeCode(
  client: SigningCosmWasmClient,
  filepath: string,
  address: string
): Promise<number> {
  const wasm: Uint8Array = fs.readFileSync(filepath);

  const result = await client.upload(address, wasm, "auto");

  return result.codeId;
}

/**
 * @notice Return the native token balance of the specified account
 */
export async function queryNativeTokenBalance(
  client: SigningCosmWasmClient,
  account: string,
  denom: string = "umockusdc"
) {
  const balance = client.getBalance(account, denom);

  if (balance) {
    return (await balance).amount;
  } else {
    return "0";
  }
}

/**
 * @notice Return CW20 token balance of the specified account
 */
export async function queryTokenBalance(
  client: SigningCosmWasmClient,
  account: string,
  contract: string
) {
  const balanceResponse = await client.queryContractSmart(contract, {
    balance: { address: account },
  });
  return balanceResponse.balance;
}

export async function retryQueryTokenBalance(
  client: SigningCosmWasmClient,
  account: string,
  contract: string
): Promise<any> {
  try {
    return await queryTokenBalance(client, account, contract);
  } catch (error) {
    sleep(10000);
    return await queryTokenBalance(client, account, contract);
  }
}

function sleep(milliseconds: number) {
  console.log("sleeping");
  const date = Date.now();
  let currentDate = null;
  // do {
  //   currentDate = Date.now();
  // } while (currentDate - date < milliseconds);
  console.log("awake");
}

export async function retryStoreCode(
  client: SigningCosmWasmClient,
  filepath: string,
  address: string
): Promise<number> {
  try {
    return await storeCode(client, filepath, address);
  } catch (error) {
    sleep(1000);
    return await storeCode(client, filepath, address);
  }
}

export async function retrySendTransaction(
  client: SigningCosmWasmClient,
  sender: string,
  contract: string,
  msg: Record<string, any>
): Promise<any> {
  try {
    return await client.execute(sender, contract, msg, "auto");
  } catch (error) {
    sleep(1000);
    return await client.execute(sender, contract, msg, "auto");
  }
}

export async function retryQuery(
  client: SigningCosmWasmClient,
  contractAddress: string,
  query: Record<string, unknown>
): Promise<any> {
  try {
    return await client.queryContractSmart(contractAddress, query);
  } catch (error) {
    sleep(10000);
    return await client.queryContractSmart(contractAddress, query);
  }
}
