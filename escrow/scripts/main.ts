import { BankAPI, Coin, isTxError, LocalTerra, MsgExecuteContract, Numeric, Wallet } from "@terra-money/terra.js";
import {expect } from 'chai';
import { assert } from "console";
import { storeCode, instantiateContract, sendTransaction, toEncodedBinary, queryTokenBalance } from "./helpers";

const terra = new LocalTerra();
const deployer = terra.wallets.test1;
const user1 = terra.wallets.test2;

let contractAddress: string;
let mirrorToken: string;

async function initTest() {
    process.stdout.write("Uploading contract...")
    const contractCodeId = await storeCode(terra, deployer, '../artifacts/escrow.wasm');
    console.log(`Done! Code Id: ${contractCodeId}`);

    // Init-ing contract
    process.stdout.write("Instantiating contract...")
    const initContract = await instantiateContract(terra, deployer, deployer, contractCodeId, {
        shop: 'shop_contract_address'
    });
    console.log(`Done!`);
    contractAddress = initContract.logs[0].events[0].attributes[0].value;
    console.log(contractAddress)
}

async function deployCw20AndMint(to: Wallet) {
    const cw20CodeId = await storeCode(
        terra,
        deployer,
        "../artifacts/terraswap_token.wasm"
      );
      const tokenResult = await instantiateContract(terra, deployer, deployer, cw20CodeId, {
        name: "Mock Mirror Token",
        symbol: "MIR",
        decimals: 6,
        initial_balances: [],
        mint: {
          minter: deployer.key.accAddress,
        },
      });
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

async function testCreatePaymentRequest() {
    const result = await sendTransaction(terra, user1, [
        new MsgExecuteContract(user1.key.accAddress, contractAddress, {
            create_payment_request: {
                asset: {
                    info :{
                        native_token: { denom: 'uusd'},
                    },
                    amount: "1000000",
                },
                order_id: "123",
            },
          }),
    ], false);
    expect(result.logs[0].events[3].attributes[1].value).to.eq("1")
}

async function testPayPaymentRequest() {
    const result = await sendTransaction(terra, user1, [
        new MsgExecuteContract(user1.key.accAddress, contractAddress, {
            pay_into_payment_request: {
                id: "1"
            },
          }, [new Coin('uusd', "1000000")]),
    ], false);
}

async function testPayPaymentRequestWithCw20() {
    const result = await sendTransaction(terra, user1, [
        new MsgExecuteContract(user1.key.accAddress, contractAddress, {
            create_payment_request: {
                asset: {
                    info :{
                        token: { contract_addr: mirrorToken},
                    },
                    amount: "10000000",
                },
                order_id: "123",
            },
          }),
    ], false);
    const prId = result.logs[0].events[3].attributes[1].value;
    const cw20Amount = await queryTokenBalance(terra, user1.key.accAddress, mirrorToken)
    await sendTransaction(terra, user1, [
        new MsgExecuteContract(user1.key.accAddress, mirrorToken, {
            send: {
                amount: "10000000",
                contract: contractAddress,
                msg: toEncodedBinary({
                  pay_into_payment_request: {
                      id: prId
                  },
                }),
            },
        })
    ])
    const cw20AmountNew = await queryTokenBalance(terra, user1.key.accAddress, mirrorToken)
    expect(cw20Amount).to.not.eq(cw20AmountNew);

    const pr = await terra.wasm.contractQuery<any>(contractAddress, {
        get_payment_request_by_id: {
            id: prId
        }
    })
    expect(pr.payment_request.paid_amount).to.eq("10000000");
}

async function testSettlementWithCw20() {
    const result = await sendTransaction(terra, deployer, [
        new MsgExecuteContract(deployer.key.accAddress, contractAddress, {
            create_payment_request: {
                asset: {
                    info :{
                        token: { contract_addr: mirrorToken},
                    },
                    amount: "10000000",
                },
                order_id: "123",
            },
          }),
    ], false);
    const prId = result.logs[0].events[3].attributes[1].value;
    const cw20Amount = await queryTokenBalance(terra, deployer.key.accAddress, mirrorToken)
    await sendTransaction(terra, user1, [
        new MsgExecuteContract(user1.key.accAddress, mirrorToken, {
            send: {
                amount: "10000000",
                contract: contractAddress,
                msg: toEncodedBinary({
                  pay_into_payment_request: {
                      id: prId
                  },
                }),
            },
        })
    ])
    await sendTransaction(terra, user1, [
        new MsgExecuteContract(user1.key.accAddress, contractAddress, {
            settle_payment_request: {
                id: prId    
            }
        })
    ])
    const cw20AmountNew = await queryTokenBalance(terra, deployer.key.accAddress, mirrorToken)
    expect(cw20Amount).to.not.eq(cw20AmountNew);
}

async function testSettlePaymentRequest() {
    const result = await sendTransaction(terra, user1, [
        new MsgExecuteContract(user1.key.accAddress, contractAddress, {
            create_payment_request: {
                asset: {
                    info :{
                        native_token: { denom: 'uusd'},
                    },
                    amount: "10000000",
                },
                order_id: "123",
            },
          }),
    ], false);
    const prId = result.logs[0].events[3].attributes[1].value;
    const bank = new BankAPI(terra.apiRequester);
    const coins = await bank.balance(user1.key.accAddress);
    const usdBalance = coins[0].get('uusd')!.amount;
    await sendTransaction(terra, deployer, [
        new MsgExecuteContract(deployer.key.accAddress, contractAddress, {
            pay_into_payment_request: {
                id: prId
            },
        }, [new Coin('uusd', "10000000")]),
    ], false);
    await sendTransaction(terra, deployer, [
        new MsgExecuteContract(deployer.key.accAddress, contractAddress, {
            settle_payment_request: {
                id: prId
            },
        }),
    ], false);

    const newCoins = await bank.balance(user1.key.accAddress);
    const newUsdBalance = newCoins[0].get('uusd')!.amount;
    expect(newUsdBalance.minus(usdBalance).toString()).to.eq("10000000");
}

async function testPayPaymentRequestWithLessAmount() {
    const result = await sendTransaction(terra, user1, [
        new MsgExecuteContract(user1.key.accAddress, contractAddress, {
            create_payment_request: {
                asset: {
                    info :{
                        native_token: { denom: 'uluna'},
                    },
                    amount: "1000000",
                },
                order_id: "123",
            },
          }),
    ], false);
    const prId = result.logs[0].events[3].attributes[1].value;
    try {
        await sendTransaction(terra, user1, [
            new MsgExecuteContract(user1.key.accAddress, contractAddress, {
                pay_into_payment_request: {
                    id: prId
                },
            }, [new Coin('uluna', "999999")]),
        ], false);
    } catch (e) {
        return
    }
}


(async () => {
    await deployCw20AndMint(user1);
    await initTest();
    await testCreatePaymentRequest();
    await testPayPaymentRequest();
    await testPayPaymentRequestWithLessAmount();
    await testSettlePaymentRequest();
    await testPayPaymentRequestWithCw20();
    await testSettlementWithCw20();
})()