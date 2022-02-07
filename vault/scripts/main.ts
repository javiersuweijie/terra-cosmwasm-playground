import {
  BankAPI,
  Coin,
  isTxError,
  LocalTerra,
  MsgExecuteContract,
  Numeric,
  Wallet,
} from "@terra-money/terra.js";
import { expect } from "chai";
import { assert } from "console";
import {
  storeCode,
  instantiateContract,
  sendTransaction,
  toEncodedBinary,
  queryTokenBalance,
} from "./helpers";

const terra = new LocalTerra();
const deployer = terra.wallets.test1;
const user1 = terra.wallets.test2;

let contractAddress: string;
let vaultToken: string;
let cw20CodeId: number;
let mirrorToken: string;

let workerToken: string;
let workerContractAddress: string;

async function initTest() {
  process.stdout.write("Uploading contract...");
  const contractCodeId = await storeCode(
    terra,
    deployer,
    "../artifacts/vault.wasm"
  );
  console.log(`Done! Code Id: ${contractCodeId}`);

  // Init-ing contract
  process.stdout.write("Instantiating contract...");
  const initMsg = {
    asset_info: {
      token: { contract_addr: mirrorToken },
    },
    reserve_pool_bps: 500,
    cw20_code_id: cw20CodeId,
  };
  const initContract = await instantiateContract(
    terra,
    deployer,
    deployer,
    contractCodeId,
    initMsg
  );
  console.log(`Done!`);
  contractAddress = initContract.logs[0].events[0].attributes[0].value;
  vaultToken = initContract.logs[0].events[3].attributes[1].value;
  console.log("Vault Contract:", contractAddress);

  // Init-ing worker
  process.stdout.write("Instantiating worker...");
  const workerContract = await instantiateContract(
    terra,
    deployer,
    deployer,
    contractCodeId,
    initMsg
  );
  console.log(`Done!`);
  workerContractAddress = workerContract.logs[0].events[0].attributes[0].value;
  workerToken = workerContract.logs[0].events[3].attributes[1].value;
  console.log("Worker Contract:", workerContractAddress);

  const vaultConfig = await terra.wasm.contractQuery<any>(contractAddress, {
    get_vault_config: {},
  });

  expect(vaultConfig.vault_token_addr).to.eq(vaultToken);
  expect(vaultConfig.asset_info.token.contract_addr).to.eq(mirrorToken);
}

async function deployCw20AndMint(to: Wallet) {
  cw20CodeId = await storeCode(
    terra,
    deployer,
    "../artifacts/terraswap_token.wasm"
  );
  const tokenResult = await instantiateContract(
    terra,
    deployer,
    deployer,
    cw20CodeId,
    {
      name: "Mock Mirror Token",
      symbol: "MIR",
      decimals: 6,
      initial_balances: [],
      mint: {
        minter: deployer.key.accAddress,
      },
    }
  );
  mirrorToken = tokenResult.logs[0].events[0].attributes[3].value;
  await sendTransaction(terra, deployer, [
    new MsgExecuteContract(deployer.key.accAddress, mirrorToken, {
      mint: {
        recipient: to.key.accAddress,
        amount: "10000000000",
      },
    }),
  ]);
}

async function testDepositWithCw20() {
  await sendTransaction(terra, user1, [
    new MsgExecuteContract(user1.key.accAddress, mirrorToken, {
      send: {
        amount: "10000000",
        contract: contractAddress,
        msg: toEncodedBinary({
          deposit: {},
        }),
      },
    }),
  ]);

  let vaultTokenAmount = await queryTokenBalance(
    terra,
    user1.key.accAddress,
    vaultToken
  );
  expect(vaultTokenAmount).to.eq("10000000");

  await sendTransaction(terra, user1, [
    new MsgExecuteContract(user1.key.accAddress, mirrorToken, {
      send: {
        amount: "5000000",
        contract: contractAddress,
        msg: toEncodedBinary({
          deposit: {},
        }),
      },
    }),
  ]);

  vaultTokenAmount = await queryTokenBalance(
    terra,
    user1.key.accAddress,
    vaultToken
  );
  expect(vaultTokenAmount).to.eq("15000000");

  const vaultBalance = await queryTokenBalance(
    terra,
    contractAddress,
    mirrorToken
  );
  expect(vaultBalance).to.eq("15000000");
}

async function testCw20Withdrawal() {
  await sendTransaction(terra, user1, [
    new MsgExecuteContract(user1.key.accAddress, vaultToken, {
      send: {
        amount: "10000000",
        contract: contractAddress,
        msg: toEncodedBinary({
          withdraw: {},
        }),
      },
    }),
  ]);

  let vaultTokenAmount = await queryTokenBalance(
    terra,
    user1.key.accAddress,
    vaultToken
  );
  expect(vaultTokenAmount).to.eq("5000000");

  await sendTransaction(terra, user1, [
    new MsgExecuteContract(user1.key.accAddress, vaultToken, {
      send: {
        amount: "5000000",
        contract: contractAddress,
        msg: toEncodedBinary({
          withdraw: {},
        }),
      },
    }),
  ]);

  vaultTokenAmount = await queryTokenBalance(
    terra,
    user1.key.accAddress,
    vaultToken
  );
  expect(vaultTokenAmount).to.eq("0");

  const vaultBalance = await queryTokenBalance(
    terra,
    contractAddress,
    mirrorToken
  );
  expect(vaultBalance).to.eq("0");
}

async function createPosition(
  user: Wallet,
  principalAmount: string,
  borrowAmount: string
) {
  const borrowResponse = await sendTransaction(terra, user, [
    new MsgExecuteContract(user.key.accAddress, mirrorToken, {
      send: {
        amount: principalAmount,
        contract: contractAddress,
        msg: toEncodedBinary({
          borrow: {
            worker_addr: workerContractAddress,
            borrow_amount: borrowAmount,
          },
        }),
      },
    }),
  ]);
  let positionId = borrowResponse.logs[0].events[3].attributes[6].value;
  return positionId;
}

async function testBorrowCw20() {
  await sendTransaction(terra, user1, [
    new MsgExecuteContract(user1.key.accAddress, mirrorToken, {
      send: {
        amount: "100000000",
        contract: contractAddress,
        msg: toEncodedBinary({
          deposit: {},
        }),
      },
    }),
  ]);
  let positionId = await createPosition(user1, "10000000", "10000000");
  let workerTokenAmount = await queryTokenBalance(
    terra,
    workerContractAddress,
    mirrorToken
  );
  expect(workerTokenAmount).to.eq("20000000");

  let contractTokenAmount = await queryTokenBalance(
    terra,
    contractAddress,
    mirrorToken
  );
  expect(contractTokenAmount).to.eq("90000000");

  let vaultConfig = await terra.wasm.contractQuery<any>(contractAddress, {
    get_vault_config: {},
  });
  expect(vaultConfig.total_debt_shares).to.eq("10000000");
  expect(vaultConfig.total_debt).to.eq("10000000");

  let position = await terra.wasm.contractQuery<any>(contractAddress, {
    get_position: { position_id: positionId },
  });
  expect(position.debt_share).to.eq("10000000");
  expect(position.owner).to.eq(user1.key.accAddress);
  expect(position.worker).to.eq(workerContractAddress);

  // Create a new position
  positionId = await createPosition(user1, "10000000", "15000000");
  vaultConfig = await terra.wasm.contractQuery<any>(contractAddress, {
    get_vault_config: {},
  });
  expect(vaultConfig.total_debt_shares).to.eq("25000000");
  expect(vaultConfig.total_debt).to.eq("25000000");

  position = await terra.wasm.contractQuery<any>(contractAddress, {
    get_position: { position_id: positionId },
  });
  expect(position.debt_share).to.eq("15000000");
  expect(position.owner).to.eq(user1.key.accAddress);
  expect(position.worker).to.eq(workerContractAddress);
}

async function testSettlementWithCw20() {
  const result = await sendTransaction(
    terra,
    deployer,
    [
      new MsgExecuteContract(deployer.key.accAddress, contractAddress, {
        create_payment_request: {
          asset: {
            info: {
              token: { contract_addr: mirrorToken },
            },
            amount: "10000000",
          },
          order_id: "123",
        },
      }),
    ],
    false
  );
  const prId = result.logs[0].events[3].attributes[1].value;
  const cw20Amount = await queryTokenBalance(
    terra,
    deployer.key.accAddress,
    mirrorToken
  );
  await sendTransaction(terra, user1, [
    new MsgExecuteContract(user1.key.accAddress, mirrorToken, {
      send: {
        amount: "10000000",
        contract: contractAddress,
        msg: toEncodedBinary({
          pay_into_payment_request: {
            id: prId,
          },
        }),
      },
    }),
  ]);
  await sendTransaction(terra, user1, [
    new MsgExecuteContract(user1.key.accAddress, contractAddress, {
      settle_payment_request: {
        id: prId,
      },
    }),
  ]);
  const cw20AmountNew = await queryTokenBalance(
    terra,
    deployer.key.accAddress,
    mirrorToken
  );
  expect(cw20Amount).to.not.eq(cw20AmountNew);
}

async function testSettlePaymentRequest() {
  const result = await sendTransaction(
    terra,
    user1,
    [
      new MsgExecuteContract(user1.key.accAddress, contractAddress, {
        create_payment_request: {
          asset: {
            info: {
              native_token: { denom: "uusd" },
            },
            amount: "10000000",
          },
          order_id: "123",
        },
      }),
    ],
    false
  );
  const prId = result.logs[0].events[3].attributes[1].value;
  const bank = new BankAPI(terra.apiRequester);
  const coins = await bank.balance(user1.key.accAddress);
  const usdBalance = coins[0].get("uusd")!.amount;
  await sendTransaction(
    terra,
    deployer,
    [
      new MsgExecuteContract(
        deployer.key.accAddress,
        contractAddress,
        {
          pay_into_payment_request: {
            id: prId,
          },
        },
        [new Coin("uusd", "10000000")]
      ),
    ],
    false
  );
  await sendTransaction(
    terra,
    deployer,
    [
      new MsgExecuteContract(deployer.key.accAddress, contractAddress, {
        settle_payment_request: {
          id: prId,
        },
      }),
    ],
    false
  );

  const newCoins = await bank.balance(user1.key.accAddress);
  const newUsdBalance = newCoins[0].get("uusd")!.amount;
  expect(newUsdBalance.minus(usdBalance).toString()).to.eq("10000000");
}

async function testPayPaymentRequestWithLessAmount() {
  const result = await sendTransaction(
    terra,
    user1,
    [
      new MsgExecuteContract(user1.key.accAddress, contractAddress, {
        create_payment_request: {
          asset: {
            info: {
              native_token: { denom: "uluna" },
            },
            amount: "1000000",
          },
          order_id: "123",
        },
      }),
    ],
    false
  );
  const prId = result.logs[0].events[3].attributes[1].value;
  try {
    await sendTransaction(
      terra,
      user1,
      [
        new MsgExecuteContract(
          user1.key.accAddress,
          contractAddress,
          {
            pay_into_payment_request: {
              id: prId,
            },
          },
          [new Coin("uluna", "999999")]
        ),
      ],
      false
    );
  } catch (e) {
    return;
  }
}

(async () => {
  await deployCw20AndMint(user1);
  await initTest();
  await testDepositWithCw20();
  await testCw20Withdrawal();
  await testBorrowCw20();
})();
