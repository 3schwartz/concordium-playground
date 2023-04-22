import { Network } from "@concordium/react-components";
import Schema from "../schema.json"
import Key from "../keys.json"
import { Buffer } from "buffer/";
import { toBuffer } from "@concordium/web-sdk";

export const DEFAULT_VERIFY_KEY: string = Key.verify_key;
export const VERIFIER_URL: string = "http://localhost:8020/api";

export const MODULE_REF: string = "6054d082a164b637d739d0ef110aa43a3cb9dd2d612ca24a8c01f5c313daf318";
export const CONTRACT_NAME: string = "dino_auction";
export const CONTRACT_TOKEN_BYTE_SIZE: number = 4;

export const MAX_CONTRACT_EXECUTION_ENERGY: bigint = BigInt(30_000);

export const SCHEMA_RAW: string = Schema.schema;
export const SCHEMA_AS_BUFFER: Buffer = toBuffer(SCHEMA_RAW, "base64");

const TESTNET_GENESIS_BLOCK_HASH: string = "4221332d34e1694168c2a0c0b3fd0f273809612cb13d000d5c2e00e85f50f796";

export const TESTNET: Network = {
    name: "testnet",
    genesisHash: TESTNET_GENESIS_BLOCK_HASH,
    jsonRpcUrl: "https://json-rpc.testnet.concordium.com",
    ccdScanBaseUrl: 'https://testnet.ccdscan.io',
}