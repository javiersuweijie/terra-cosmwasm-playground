import { LocalTerra, LCDClient, Wallet, Msg, Coin, isTxError, MsgStoreCode, MsgInstantiateContract, Fee } from "@terra-money/terra.js";
import chalk from "chalk";
import * as fs from "fs";

/**
 * @notice Encode a JSON object to base64 binary
 */
 export function toEncodedBinary(obj: any) {
    return Buffer.from(JSON.stringify(obj)).toString("base64");
  }
  
  /**
   * @notice Send a transaction. Return result if successful, throw error if failed.
   */
  export async function sendTransaction(
    terra: LocalTerra | LCDClient,
    sender: Wallet,
    msgs: Msg[],
    verbose = false
  ) {
    const tx = await sender.createAndSignTx({
      msgs,
      fee: new Fee(30000000, [new Coin("uluna", 4500000), new Coin("uusd", 4500000)]),
    });
  
    const result = await terra.tx.broadcast(tx);
  
    // Print the log info
    if (verbose) {
      console.log(chalk.magenta("\nTxHash:"), result.txhash);
      try {
        console.log(
          chalk.magenta("Raw log:"),
          JSON.stringify(JSON.parse(result.raw_log), null, 2)
        );
      } catch {
        console.log(chalk.magenta("Failed to parse log! Raw log:"), result.raw_log);
      }
    }
  
    if (isTxError(result)) {
      throw new Error(
        chalk.red("Transaction failed!") +
          `\n${chalk.yellow("code")}: ${result.code}` +
          `\n${chalk.yellow("codespace")}: ${result.codespace}` +
          `\n${chalk.yellow("raw_log")}: ${result.raw_log}`
      );
    }
  
    return result;
  }
  
  /**
   * @notice Upload contract code to LocalTerra. Return code ID.
   */
  export async function storeCode(
    terra: LocalTerra | LCDClient,
    deployer: Wallet,
    filepath: string
  ) {
    const code = fs.readFileSync(filepath).toString("base64");
    const result = await sendTransaction(terra, deployer, [
      new MsgStoreCode(deployer.key.accAddress, code),
    ], true);
    return parseInt(result.logs[0].eventsByType.store_code.code_id[0]);
  }
  
  /**
   * @notice Instantiate a contract from an existing code ID. Return contract address.
   */
  export async function instantiateContract(
    terra: LocalTerra | LCDClient,
    deployer: Wallet,
    admin: Wallet, // leave this emtpy then contract is not migratable
    codeId: number,
    instantiateMsg: object
  ) {
    const result = await sendTransaction(terra, deployer, [
      new MsgInstantiateContract(
        deployer.key.accAddress,
        admin.key.accAddress,
        codeId,
        instantiateMsg
      ),
    ]);
    return result;
  }