import chalk from "chalk";
import * as chai from "chai";
import chaiAsPromised from "chai-as-promised";
import { Decimal } from "@cosmjs/math";

import { DirectSecp256k1HdWallet } from "@cosmjs/proto-signing";

import {
  SigningCosmWasmClient,
  SigningCosmWasmClientOptions,
} from "@cosmjs/cosmwasm-stargate";

import { setUpPairs } from "./testing/setup";

chai.use(chaiAsPromised);

//----------------------------------------------------------------------------------------
// Variables
//----------------------------------------------------------------------------------------

async function setupTest() {
  const mnemonic =
    "satisfy adjust timber high purchase tuition stool faith fine install that you unaware feed domain license impose boss human eager hat rent enjoy dawn";
  const wallet = await DirectSecp256k1HdWallet.fromMnemonic(mnemonic, {
    prefix: "juno",
  });

  const [firstAccount] = await wallet.getAccounts();
  console.log(firstAccount.address);

  const rpcEndpoint = "http://localhost:26657/";

  const clientOptions: SigningCosmWasmClientOptions = {
    gasPrice: { denom: "ujunox", amount: Decimal.fromUserInput("0", 0) },
  };

  const client = await SigningCosmWasmClient.connectWithSigner(
    rpcEndpoint,
    wallet,
    clientOptions
  );

  const mockUsdcDenom = "umockusdc";
  await client.getBalance(firstAccount.address, "ujunox").then((res) => {
    console.log(res);
  });

  await client.getBalance(firstAccount.address, mockUsdcDenom).then((res) => {
    console.log(res);
  });

  const setPairParams = await setUpPairs(client, firstAccount.address);
  console.log(setPairParams);
}

//----------------------------------------------------------------------------------------
// Main
//----------------------------------------------------------------------------------------

(async () => {
  console.log(chalk.yellow("\nStep 1. Setup"));

  await setupTest();
})();
