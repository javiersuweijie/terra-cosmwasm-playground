import { isTxError, LocalTerra, MsgExecuteContract } from "@terra-money/terra.js";
import {expect } from 'chai';
import { storeCode, instantiateContract, sendTransaction, toEncodedBinary } from "./helpers";

const terra = new LocalTerra();
const deployer = terra.wallets.test1;
const user1 = terra.wallets.test2;

let contractAddress: string;

async function initTest() {
    process.stdout.write("Uploading contract...")
    const contractCodeId = await storeCode(terra, deployer, '../artifacts/sps.wasm');
    console.log(`Done! Code Id: ${contractCodeId}`);

    // Init-ing contract
    process.stdout.write("Instantiating contract...")
    const initContract = await instantiateContract(terra, deployer, deployer, contractCodeId, {});
    console.log(`Done!`);
    contractAddress = initContract.logs[0].events[0].attributes[0].value;
    console.log(contractAddress)
}

async function assertQueryCount(expected: number) {
    try {
    const countResponse = await terra.wasm.contractQuery<{ count: number }>(contractAddress, {
        get_count: {}
    });
    expect(countResponse.count).to.equal(expected);
    } catch (e) {
        console.error(e)
    }
}

async function assertQueryOwner(expected: string) {
    try {
        const ownerResponse = await terra.wasm.contractQuery<{ owner: string }>(contractAddress, {
            get_owner: {}
        });
        console.log(`owner is: ${ownerResponse.owner}`)
        expect(ownerResponse.owner).to.equal(expected);
    } catch (e) {
        console.error(e)
    }
}

async function testStartGame() {
    await sendTransaction(terra, deployer, [
        new MsgExecuteContract(deployer.key.accAddress, contractAddress, {
            start_game: {
                opponent: user1.key.accAddress,
                action: {Paper: {}},
            },
          }),
    ], false);

    const gameStates = await terra.wasm.contractQuery<{
        games: {
        host: string,
        opponent: string,
    }[]}>(contractAddress, {
        get_game_states_by_host: {
            host: deployer.key.accAddress
        }
    })
    expect(gameStates.games[0].opponent).to.eq(user1.key.accAddress)
}

async function testDrawGame() {
    const result = await sendTransaction(terra, user1, [
        new MsgExecuteContract(user1.key.accAddress, contractAddress, {
            opponent_move: {
                host: deployer.key.accAddress,
                action: {Paper: {}},
            },
          }),
    ], false);
    expect(result.logs[0].events[3].attributes[1].value).to.eq('draw')
}

async function testWinGame() {
    const result = await sendTransaction(terra, user1, [
        new MsgExecuteContract(user1.key.accAddress, contractAddress, {
            opponent_move: {
                host: deployer.key.accAddress,
                action: {Scissors: {}},
            },
          }),
    ], false);
    expect(result.logs[0].events[3].attributes[2].value).to.eq(user1.key.accAddress)
}

async function testReset() {
    await sendTransaction(terra, deployer, [
        new MsgExecuteContract(deployer.key.accAddress, contractAddress, {
            reset: {count: 10},
          }),
    ], false);
    await assertQueryCount(10);
}

async function testUnauthorizedReset() {
    try {
        await sendTransaction(terra, user1, [
        new MsgExecuteContract(user1.key.accAddress, contractAddress, {
            reset: {count: 10},
          }),
        ], false);
    } catch (e) {
        return
    }
    throw Error("Expected to throw unauthorized error");
}

async function testUnauthorizedChangeOwner() {
    try {
        await sendTransaction(terra, user1, [
        new MsgExecuteContract(user1.key.accAddress, contractAddress, {
            change_owner: {owner: user1.key.publicKey},
          }),
        ], false);
    } catch (e) {
        return
    }
    throw Error("Expected to throw unauthorized error");
}

async function testChangeOwner() {
    await sendTransaction(terra, deployer, [
    new MsgExecuteContract(deployer.key.accAddress, contractAddress, {
        change_owner: {owner: user1.key.accAddress},
        }),
    ], false);
    await assertQueryOwner(user1.key.publicKey!.address());
}

(async () => {
    await initTest();
    await testStartGame();
    await testDrawGame();
    await testWinGame();
})()