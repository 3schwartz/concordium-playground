import { WalletApi } from "@concordium/browser-wallet-api-helpers";
import { serializeUpdateContractParameters, InvokeContractResult, InvokeContractFailedResult, toBuffer, deserializeReceiveReturnValue, TransactionSummary, AccountTransactionType, CcdAmount, UpdateContractPayload, TransactionStatusEnum, RejectedReceive, InitContractPayload, ModuleReference, ContractAddress } from "@concordium/web-sdk";
import { CONTRACT_NAME, SCHEMA_AS_BUFFER, MAX_CONTRACT_EXECUTION_ENERGY, SCHEMA_RAW, TESTNET, MODULE_REF } from "../utils/config";

export interface ContractInitializedEvent {
    address: ContractAddress,
    amount: bigint,
    contractVersion: number,
    events: [string],
    initName: string,
    ref: string,
    tag: string
}

export const invokeContract = async function <T>(
    provider: WalletApi,
    methodName: string,
    contractId: bigint,
    parameters: any,
    setError: (error: string) => void)
    : Promise<T | undefined> {
    try {
        const param = serializeUpdateContractParameters(
            CONTRACT_NAME,
            methodName,
            parameters,
            SCHEMA_AS_BUFFER);

        const tokenResult: InvokeContractResult | undefined = await provider.getJsonRpcClient()
            .invokeContract({
                contract: { index: contractId, subindex: 0n },
                method: `${CONTRACT_NAME}.${methodName}`,
                parameter: param
            });

        if (!tokenResult) {
            throw Error(`Unable to get ${methodName}`);
        }
        if (tokenResult.tag === "failure") {
            const result = tokenResult as InvokeContractFailedResult;
            throw Error(`${result.reason.tag}${
                result.reason.tag === "RejectedReceive" ?
                `: ${(result.reason as RejectedReceive).rejectReason}`
                :
                ""
            }`);
        }

        const buffer = toBuffer(tokenResult.returnValue || "", "hex");
        const response: T = deserializeReceiveReturnValue(
            buffer, SCHEMA_AS_BUFFER, CONTRACT_NAME, methodName);

        return response;
    } catch (error: any) {
        setError((error as Error).message);
    }
}

export const updateContract = async function (
    provider: WalletApi,
    contractId: bigint,
    amount: bigint,
    account: string,
    methodName: string,
    params: any,
): Promise<Record<string, TransactionSummary>> {
    const txHash = await provider.sendTransaction(
        account,
        AccountTransactionType.Update,
        {
            amount: new CcdAmount(amount),
            address: {
                index: contractId,
                subindex: 0n
            },
            receiveName: `${CONTRACT_NAME}.${methodName}`,
            maxContractExecutionEnergy: MAX_CONTRACT_EXECUTION_ENERGY
        } as UpdateContractPayload,
        params,
        SCHEMA_RAW
    );
    logTransaction(txHash);
    return new Promise((res, rej) => {
        wait(provider, txHash, res, rej);
    });
}

export const initContract = async function (
    provider: WalletApi,
    amount: bigint,
    account: string,
    params: any,
): Promise<ContractInitializedEvent> {
    const txHash = await provider.sendTransaction(
        account,
        AccountTransactionType.InitContract,
        {
            amount: new CcdAmount(amount),
            moduleRef: new ModuleReference(MODULE_REF),
            initName: CONTRACT_NAME,
            maxContractExecutionEnergy: MAX_CONTRACT_EXECUTION_ENERGY
        } as InitContractPayload,
        params,
        SCHEMA_RAW
    );
    logTransaction(txHash);
    return new Promise((res, rej) => {
        wait(provider, txHash, res, rej);
    }).then((value: unknown) => {
        const result = Object.values(value!)[0];
        return result.result.events[0] as ContractInitializedEvent;
    });
}

export function jsonStringify(error: any): string {
    return JSON.stringify(error, (key, value) => {
        if (key === "contractAddress") {
          return undefined;
        }
        return value;
    }, 2);
}

function wait(
    provider: WalletApi, 
    txHash: string,
    res: (p: Record<string, TransactionSummary>) => void,
    rej: (reason: any) => void) {
        setTimeout(() => {
            provider
                .getJsonRpcClient()
                .getTransactionStatus(txHash)
                .then((txStatus) => {
                    if (!txStatus) {
                        return rej("Transaction is null");
                    }
                    if (txStatus.status === TransactionStatusEnum.Finalized) {

                        const result = Object.values(txStatus.outcomes!)[0];
                        
                        if (result.result.outcome === "success") {
                            return res(txStatus.outcomes!);
                        }

                        return rej(result.result.rejectReason);
                    }

                    wait(provider, txHash, res, rej);
                })
                .catch((err) => rej(err));
        }, 1_000);
}

const logTransaction = (txHash: string): void => {
    console.debug(`${TESTNET.ccdScanBaseUrl}/?dcount=1&dentity=transaction&dhash=${txHash}`);
}