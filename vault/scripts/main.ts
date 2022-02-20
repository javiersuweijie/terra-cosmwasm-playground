import {
  BankAPI,
  Coin,
  Coins,
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
const astroFactory = "terra1kyl8f2xkd63cga8szgkejdyvxay7mc7qpdc3c5";

let mirUsdPoolAddress: string;
let midUsdLpAddress: string;
let contractAddress: string;
let vaultToken: string;
let cw20CodeId: number;
let mirrorToken: string;

let farmContractAddress: string;

async function main() {
  await deployCw20AndMint(user1);
  await initTest();
  await testDepositWithCw20();
  await testCw20Withdrawal();
  await testOpenPosition();
  //   await testBorrowCw20();
}

async function initTest() {
  process.stdout.write("Uploading contract...");
  const contractCodeId = await storeCode(
    terra,
    deployer,
    "../artifacts/vault.wasm"
  );
  console.log(`Done! Code Id: ${contractCodeId}`);
  process.stdout.write("Uploading farm...");
  const farmCodeId = await storeCode(terra, deployer, "../artifacts/farm.wasm");
  console.log(`Done! Code Id: ${farmCodeId}`);

  // Init-ing contract
  process.stdout.write("Instantiating contract...");
  const initMsg = {
    asset_info: {
      token: { contract_addr: mirrorToken },
    },
    reserve_pool_bps: 500,
    cw20_code_id: cw20CodeId,
    whitelisted_farms: [],
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
  process.stdout.write("Instantiating farm...");
  const workerContract = await instantiateContract(
    terra,
    deployer,
    deployer,
    farmCodeId,
    {
      vault_addr: contractAddress,
      base_asset: {
        token: { contract_addr: mirrorToken },
      },
      other_asset: {
        native_token: { denom: "uusd" },
      },
      claim_asset_addr: midUsdLpAddress,
      astroport_factory_addr: astroFactory,
    }
  );
  console.log(`Done!`);
  farmContractAddress = workerContract.logs[0].events[0].attributes[3].value;
  console.log("Farm Contract:", farmContractAddress);

  const farmConfig = await terra.wasm.contractQuery<any>(farmContractAddress, {
    get_farm: {},
  });

  console.log(farmConfig);

  await sendTransaction(terra, deployer, [
    new MsgExecuteContract(deployer.key.accAddress, contractAddress, {
      add_whitelist: {
        address: farmContractAddress,
      },
    }),
  ]);

  const vaultConfig = await terra.wasm.contractQuery<any>(contractAddress, {
    get_vault_config: {},
  });

  expect(vaultConfig.vault_token_addr).to.eq(vaultToken);
  expect(vaultConfig.asset_info.token.contract_addr).to.eq(mirrorToken);
  expect(vaultConfig.whitelisted_farms[0]).to.equal(farmContractAddress);

  const pairInfo = await terra.wasm.contractQuery<any>(astroFactory, {
    pair: {
      asset_infos: [
        {
          token: {
            contract_addr: mirrorToken,
          },
        },
        { native_token: { denom: "uusd" } },
      ],
    },
  });
  console.log(pairInfo);
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

  // Create astroport pool
  const pool = await sendTransaction(terra, deployer, [
    new MsgExecuteContract(deployer.key.accAddress, astroFactory, {
      create_pair: {
        pair_type: { xyk: {} },
        asset_infos: [
          {
            token: { contract_addr: mirrorToken },
          },
          {
            native_token: { denom: "uusd" },
          },
        ],
      },
    }),
  ]);
  console.log(pool);
  mirUsdPoolAddress = pool.logs[0].events[4].attributes[7].value;
  midUsdLpAddress = pool.logs[0].events[2].attributes[7].value;
  const provide = await sendTransaction(terra, to, [
    new MsgExecuteContract(to.key.accAddress, mirrorToken, {
      increase_allowance: {
        amount: "10000000",
        spender: mirUsdPoolAddress,
      },
    }),
    new MsgExecuteContract(
      to.key.accAddress,
      mirUsdPoolAddress,
      {
        provide_liquidity: {
          assets: [
            {
              info: { token: { contract_addr: mirrorToken } },
              amount: "5000000",
            },
            {
              info: { native_token: { denom: "uusd" } },
              amount: "10000000",
            },
          ],
        },
      },
      [new Coin("uusd", "10000000")]
    ),
  ]);
  console.log(provide);

  const simulation = await terra.wasm.contractQuery(mirUsdPoolAddress, {
    simulation: {
      offer_asset: {
        info: {
          token: {
            contract_addr: mirrorToken,
          },
        },
        amount: "1000000",
      },
    },
  });
  console.log(simulation);
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
      increase_allowance: {
        amount: principalAmount,
        spender: farmContractAddress,
      },
    }),
    new MsgExecuteContract(user.key.accAddress, farmContractAddress, {
      open: {
        base_asset_amount: principalAmount,
        borrow_amount: borrowAmount,
      },
    }),
  ]);
  let positionId = borrowResponse.logs[1].events[6].attributes[7].value;
  return positionId;
}

async function testOpenPosition() {
  await testDepositWithCw20();
  let positionId = await createPosition(user1, "9000", "8000");
  let workerTokenAmount = await queryTokenBalance(
    terra,
    farmContractAddress,
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
  expect(position.worker).to.eq(farmContractAddress);

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
  expect(position.worker).to.eq(farmContractAddress);
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
  await main();
})();
