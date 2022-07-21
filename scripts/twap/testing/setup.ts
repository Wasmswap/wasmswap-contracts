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
    path.resolve(__dirname, "../../../artifacts/wasmswap-aarch64.wasm"),
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

  const junopool = pairResultJuno.contractAddress;

  await client.execute(
    deployerAddress,
    junopool,
    {
      add_liquidity: {
        token1_amount: "500000",
        min_liquidity: "0",
        max_token2: "10000000",
      },
    },

    "auto",
    "",
    [
      { denom: "ujunox", amount: "500000" },
      { denom: "umockusdc", amount: "10000000" },
    ]
  );

  const twap = await getTwapPrices(client, junopool);
  console.log(twap);

  const spot = await getSpotPrices(client, junopool);
  console.log(spot);

  //   Swap {
  //     input_token: TokenSelect,
  //     input_amount: Uint128,
  //     min_output: Uint128,
  //     expiration: Option<Expiration>,
  // },

  await client.execute(
    deployerAddress,
    junopool,
    {
      swap: {
        input_token: "Token1",
        input_amount: "5000",
        min_output: "0",
      },
    },

    "auto",
    "",
    [{ denom: "ujunox", amount: "5000" }]
  );

  const twap2 = await getTwapPrices(client, junopool);
  console.log(twap2);

  const spot2 = await getSpotPrices(client, junopool);
  console.log(spot2);

  await client.execute(
    deployerAddress,
    junopool,
    {
      swap: {
        input_token: "Token1",
        input_amount: "5000",
        min_output: "0",
      },
    },

    "auto",
    "",
    [{ denom: "ujunox", amount: "5000" }]
  );

  const twap3 = await getTwapPrices(client, junopool);
  console.log(twap3);

  const spot3 = await getSpotPrices(client, junopool);
  console.log(spot3);

  await client.execute(
    deployerAddress,
    junopool,
    {
      swap: {
        input_token: "Token1",
        input_amount: "5000",
        min_output: "0",
      },
    },

    "auto",
    "",
    [{ denom: "ujunox", amount: "5000" }]
  );

  const twap4 = await getTwapPrices(client, junopool);
  console.log(twap4);

  const spot4 = await getSpotPrices(client, junopool);
  console.log(spot4);
  return {
    cw20CodeId: cw20CodeId,
    junoPair: junopool,
  };
}

async function getTwapPrices(client: SigningCosmWasmClient, contract: string) {
  return await client.queryContractSmart(contract, {
    twap_prices: {},
  });
}

async function getSpotPrices(client: SigningCosmWasmClient, contract: string) {
  const amountUSDC = Number(
    await (
      await client.getBalance(contract, "umockusdc")
    ).amount
  );
  const amountUJUNO = Number(
    await (
      await client.getBalance(contract, "ujunox")
    ).amount
  );
  return {
    junoPrice: amountUSDC / amountUJUNO,
    usdcPrice: amountUJUNO / amountUSDC,
  };
}
