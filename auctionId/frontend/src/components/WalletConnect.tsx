import { detectConcordiumProvider, EventType, WalletApi } from "@concordium/browser-wallet-api-helpers";
import { Alert, Button } from "@mui/material";
import { useCallback, useEffect, useState } from "react";

const ACCOUNT_PAGE: string = "https://testnet.ccdscan.io/?dcount=1&dentity=account&daddress=";

interface WalletConnectInfo {
    account: string | undefined
    setAccount: (account: string | undefined) => void,
    isConnecting: boolean,
    setIsConnected: (isConnected: boolean) => void,
    provider: WalletApi | undefined,
    setProvider: (provider: WalletApi | undefined) => void
}

export function WalletConnect(info: WalletConnectInfo) {
    const {
        account,
        setAccount,
        isConnecting,
        setIsConnected,
        provider,
        setProvider
    } = info;

    const [connectError, setConnectError] = useState<string>();

    const openAccountInfoPage = (account: string): void => {
        window.open(`${ACCOUNT_PAGE}${account}`,
            '_blank',
            'noopener,noreferrer')
    }

    useEffect(() => {
        return () => {
            provider?.removeAllListeners()
        };
    }, [provider])

    const connect = useCallback(() => {
        provider!.connect()
            .then((account: string | undefined) => {
                setAccount(account);
                setIsConnected(true);
                setConnectError(undefined);
            })
            .catch((err: any) => {
                setAccount(undefined);
                setIsConnected(false);
                setConnectError((err as Error).message);
            })
    }, [provider, setAccount, setIsConnected]);

    useEffect(() => {
        detectConcordiumProvider()
            .then((provider) => {
                setProvider(provider);
                setTimeout(() => {
                    provider.on(EventType.AccountChanged, (accountChange: string) => {
                        console.log(`Changed to account: ${accountChange}`);
                        setAccount(accountChange);
                    });
                    provider.on(EventType.AccountDisconnected, (accountDisconnected) => {
                        console.log(`Account disconnected: ${accountDisconnected}`);
                        setAccount(undefined);
                    });
                    provider.on(EventType.ChainChanged, (chainChange: string) => {
                        console.log(`Chain change: ${chainChange}`);
                    });
                }, 3_000)
            })
            .catch((err: any) => {
                setAccount(undefined);
                setIsConnected(false);
                setConnectError(err);
            });
    }, [setAccount, setIsConnected, setProvider]);

    return (
        <>
            {connectError && <Alert severity='error'>Connect Error: {connectError}</Alert>}
            {!account && (
                <Button
                    onClick={connect}
                    disabled={isConnecting || !provider}
                    color="secondary"
                    variant='contained'
                >
                    {isConnecting && "Connecting.."}
                    {!isConnecting && "Connect Browser Wallet"}
                </Button>
            )}
            {account && (
                <Button
                    variant="contained"
                    color="secondary"
                    onClick={() => openAccountInfoPage(account)}>
                    {account.slice(0, 4)}...{account.slice(-4)}
                </Button>
            )}
        </>
    )
}
