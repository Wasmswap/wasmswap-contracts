import * as path from "path";
import BN from "bn.js";
import chalk from "chalk";
import * as chai from "chai";
import chaiAsPromised from "chai-as-promised";

import {
  toEncodedBinary,
  storeCode,
  queryNativeTokenBalance,
  queryTokenBalance,
  retryStoreCode,
  retrySendTransaction,
  retryQuery,
} from "../helpers";
import { SigningStargateClient } from "@cosmjs/stargate";
import { SigningCosmWasmClient } from "@cosmjs/cosmwasm-stargate";

chai.use(chaiAsPromised);

export async function setUpPairs(
  client: SigningCosmWasmClient,
  deployerAddress: string
): Promise<{
  cw20CodeId: number;
  junoPair: string;
}> {
  console.log(chalk.green("Uploading cw20 base contract..."));
  const cw20CodeId = await retryStoreCode(
    client,
    path.resolve(__dirname, "../../cw20_base.wasm"),
    deployerAddress
  );

  console.log(chalk.green("Done!"), `${chalk.blue("codeId")}=${cw20CodeId}`);

  // Step 3. Upload TerraSwap Pair code
  process.stdout.write("Uploading Wasmswap pair code... ");

  const codeId = await storeCode(
    client,
    path.resolve(__dirname, "../../../artifacts/wasmswap.wasm"),
    deployerAddress
  );

  console.log(chalk.green("Done!"), `${chalk.blue("codeId")}=${codeId}`);

  // // Step 4. Instantiate TerraSwap Pair contract
  process.stdout.write("Instantiating Wasmswap pair contract... ");

  const pairResultJuno = await client.instantiate(
    deployerAddress,
    codeId,
    {
      token1_denom: { native: "ujunox" },
      token2_denom: { native: "umockusdc" },
      lp_token_code_id: cw20CodeId,
    },

    "wasmswap",
    "auto"
  );

  const terraswapPair2 = pairResultJuno.contractAddress;

  await client.execute(
    deployerAddress,
    terraswapPair2,
    {
      add_liquidity: {
        token1_amount: "100000",
        min_liquidity: "0",
        max_token2: "10000000",
      },
    },

    "auto",
    "",
    [
      { denom: "ujunox", amount: "100000" },
      { denom: "umockusdc", amount: "10000000" },
    ]
  );

  return {
    cw20CodeId: cw20CodeId,
    junoPair: terraswapPair2,
  };
}
