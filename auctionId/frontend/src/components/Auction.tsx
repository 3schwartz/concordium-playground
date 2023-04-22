import { WalletApi } from "@concordium/browser-wallet-api-helpers";
import { useCallback, useEffect, useState } from "react";
import { useParams } from "react-router";
import { invokeContract, jsonStringify, updateContract } from "../modules/contractClients";
import { Box, Button, CircularProgress, Container, Grid, Paper, Typography } from "@mui/material";
import { Login, Logout } from "@mui/icons-material";
import { IdStatementBuilder } from "@concordium/web-sdk";
import { getChallenge, getSignature } from "../modules/verifierClient";

interface AuctionInfo {
    provider: WalletApi | undefined,
    account: string | undefined,
    contractId: bigint | undefined,
    setError: (error: string) => void,
}

export function Auction(info: AuctionInfo) {
    const { provider, account, contractId, setError } = info;
    const { auctionId } = useParams();

    const [processing, setProcessing] = useState(false);
    const [isIn, setIsIn] = useState<Boolean>();

    const getIsIn = useCallback(() => {
        if (!provider || !account || !contractId) {
            return;
        }
        const params = [{
            token_id: auctionId!,
            address: {
                Account: [account!]
            }
        }]
        invokeContract<string[]>(provider, "balanceOf", contractId, params, setError)
            .then((balances: string[] | undefined) => {
                if (balances === undefined || balances.length === 0) {
                    setIsIn(undefined)
                    return;
                }
                setIsIn(balances[0] !== "0");
            });
    }, [provider, contractId, account, auctionId, setError]);

    useEffect(() => {
        getIsIn();
    }, [provider, contractId, account, getIsIn])

    const enterAuction = useCallback(async () => {
        try {
            setProcessing(true);

            const challenge = await getChallenge(account!);

            const statementBuilder = new IdStatementBuilder();
            statementBuilder.addEUNationality();
            const statement = statementBuilder.getStatement();

            const proof = await provider!.requestIdProof(account!, statement!, challenge)

            const signature = await getSignature(challenge, proof);

            const param = {
                tokens: [auctionId],
                signature
            }

            updateContract(provider!, contractId!, 0n, account!, "mint", param)
                .then(() => {
                    getIsIn();
                })
                .catch((error: any) => setError(jsonStringify(error)))
                .finally(() => setProcessing(false));
        } catch (error: any) {
            setError((error as Error).message);
            setProcessing(false);
        }
    }, [account, provider, contractId, auctionId, getIsIn, setError]);

    const leaveAuction = useCallback(async () => {
        try {
            setProcessing(true);

            const param = {
                token_id: auctionId,
            }

            updateContract(provider!, contractId!, 0n, account!, "burn", param)
                .then(() => {
                    getIsIn();
                })
                .catch((error: any) => setError(jsonStringify(error)))
                .finally(() => setProcessing(false));
        } catch (error: any) {
            setError((error as Error).message);
            setProcessing(false);
        }
    }, [account, provider, contractId, auctionId, getIsIn, setError]);

    return (
        <Container maxWidth="md">
            <Paper variant="outlined"
                sx={{ my: { xs: 3, md: 6 }, p: { xs: 2, md: 3 } }}>
                <Grid container spacing={2}>
                    <Grid item xs={12}>
                        {processing && (
                            <Box
                                sx={{
                                    display: "flex",
                                    alignItems: "center",
                                    justifyContent: "center",
                                    flexDirection: "column",
                                    height: "100hv"
                                }}
                            >
                                <Typography
                                    component="div"
                                    variant="body1"
                                    sx={{
                                        mb: 2,
                                        letterSpacing: ".3rem"
                                    }}
                                >
                                    Updating contract...
                                </Typography>
                                <CircularProgress />
                            </Box>
                        )}
                    </Grid>
                    {isIn === undefined &&
                        <Grid item xs={12}>
                            <Typography>
                                State isn't known...
                            </Typography>
                        </Grid>
                    }
                    {isIn === false &&
                        <>
                            <Grid item xs={12}>
                                <Typography variant="h5" align="center">
                                    You are not participating in auction {auctionId}
                                </Typography>
                                <Typography variant="caption">
                                    Enter auction
                                </Typography>
                            </Grid>
                            <Grid item xs={12}>

                                <Button
                                    onClick={enterAuction}
                                    fullWidth
                                    variant="contained"
                                    disabled={
                                        processing || !provider ||
                                        !account || !auctionId || !contractId}
                                >
                                    <Login />
                                </Button>
                            </Grid>
                        </>
                    }
                    {isIn &&
                        <>
                            <Grid item xs={12}>
                                <Typography variant="h5" align="center">
                                    You are participating in auction {auctionId}
                                </Typography>
                                <Typography variant="caption">
                                    Leave auction
                                </Typography>
                            </Grid>
                            <Grid item xs={12}>

                                <Button
                                    onClick={leaveAuction}
                                    fullWidth
                                    variant="contained"
                                    disabled={
                                        processing || !provider ||
                                        !account || !auctionId || !contractId}
                                >
                                    <Logout />
                                </Button>
                            </Grid>
                        </>
                    }
                </Grid>
            </Paper>
        </Container>
    )
}