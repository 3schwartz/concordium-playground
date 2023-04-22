import { WalletApi } from "@concordium/browser-wallet-api-helpers";
import { Box, Button, CircularProgress, Container, Divider, Grid, IconButton, List, ListItem, ListItemIcon, ListItemText, Paper, TextField, Typography } from "@mui/material";
import { useCallback, useEffect } from "react";
import { CONTRACT_TOKEN_BYTE_SIZE, DEFAULT_VERIFY_KEY } from "../utils/config";
import { useState } from "react";
import { Buffer } from "buffer/";
import { ContractInitializedEvent, initContract, invokeContract, jsonStringify, updateContract } from "../modules/contractClients";
import { Link as RouterLink } from 'react-router-dom';
import { Login } from "@mui/icons-material";

interface OverviewInfo {
    provider: WalletApi | undefined,
    account: string | undefined,
    contractId: bigint | undefined,
    setError: (error: string) => void,
    setContractId: (contractId: bigint) => void
}

interface View {
    tokens: string[]
}

const tokenValidate = (
    event: string,
    setTokenInputState: (token: string) => void,
    setToken: (token: string | undefined) => void,
    setTokenError: (error: string | undefined) => void) => {
    setTokenInputState(event);
    try {
        let buff = Buffer.from(event, "hex");
        const parsedHex = buff.subarray(0, CONTRACT_TOKEN_BYTE_SIZE);
        let parsedTokenIdHex = Buffer.from(parsedHex).toString("hex");
        if (event === parsedTokenIdHex) {
            setToken(("00000000" + event).slice(-8))
            setTokenError(undefined);
            return;
        }
        setTokenError(`Parsed to: ${parsedTokenIdHex}`);
    } catch (error) {
        setTokenError((error as Error).message);
    }
}

export function Overview(info: OverviewInfo) {
    const { provider, account, contractId, setError, setContractId } = info;
    const [owner, setOwner] = useState<string>();

    const [processing, setProcessing] = useState(false);

    const [view, setView] = useState<View>();

    const [tokenToBurn, setTokenToBurn] = useState<string>();
    const [tokenToBurnInput, setTokenToBurnInput] = useState("");
    const [tokenToBurnError, setTokenToBurnError] = useState<string>();

    const [tokenToInit, setTokenToInit] = useState<string>();
    const [tokenToInitInput, setTokenToInitInput] = useState("");
    const [tokenToInitError, setTokenToInitError] = useState<string>();
    const [tokenToInitQuantity, setTokenToInitQuantity] = useState(100);

    const getView = useCallback(() => {
        if (!provider || !contractId) {
            return;
        }
        invokeContract<View>(provider, "view", contractId, {}, setError)
            .then((view: View | undefined) => setView(view));
    }, [contractId, provider, setError]);

    useEffect(() => {
        if (!provider || !contractId) {
            return;
        }
        getView();

        invokeContract<string>(provider, "get_owner", contractId, {}, setError)
            .then(setOwner)
    }, [contractId, getView, provider, setError])

    const handleTokenToBurnInput = useCallback((event: string) => {
        tokenValidate(event, setTokenToBurnInput, setTokenToBurn, setTokenToBurnError);
    }, []);

    const submitBurnAuction = useCallback((event: React.FormEvent<HTMLFormElement>) => {
        event.preventDefault();
        const input = {
            tokens: [tokenToBurn!]
        };
        setProcessing(true);
        updateContract(provider!, contractId!, 0n, account!, "burn_auction", input)
            .then(() => {
                getView();
                setTokenToBurnInput("");
                setTokenToBurn(undefined);
            })
            .catch((error: any) => setError(JSON.stringify((error))))
            .finally(() => setProcessing(false));
    }, [account, contractId, getView, provider, setError, tokenToBurn]);

    const handleTokenToInitInput = useCallback((event: string) => {
        tokenValidate(event, setTokenToInitInput, setTokenToInit, setTokenToInitError);
    }, []);

    const initAuction = useCallback(async (event: React.FormEvent<HTMLFormElement>) => {
        event.preventDefault();
        const tokens = {
            [tokenToInit!]: [{ url: tokenToInit, hash: '' }, tokenToInitQuantity.toString()]
        }
        const input = {
            tokens: Object.keys(tokens).map((tokenId) => [tokenId, tokens[tokenId]])
        }

        setProcessing(true);
        updateContract(provider!, contractId!, 0n, account!, "init_auction", input)
            .then(() => {
                getView();
                setTokenToInitInput("");
                setTokenToInit(undefined);
                setTokenToInitQuantity(100);
            })
            .catch((error: any) => setError(jsonStringify(error)))
            .finally(() => setProcessing(false));

    }, [account, contractId, getView, provider, setError, tokenToInit, tokenToInitQuantity]);

    const initializeContract = useCallback(async (event: React.FormEvent<HTMLFormElement>) => {
        event.preventDefault();

        const params = {
            verify_key: DEFAULT_VERIFY_KEY
        }

        setProcessing(true);
        initContract(provider!, 0n, account!, params)
            .then((contract: ContractInitializedEvent) => {
                setContractId(contract.address.index);
            })
            .catch((error: any) => setError(jsonStringify(error)))
            .finally(() => setProcessing(false));

    }, [account, provider, setError, setContractId]);

    return (
        <Container maxWidth="md">
            <Paper variant='outlined'
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

                    {view &&
                        <>
                            <Grid item xs={12}>
                                <Typography variant="h5" align="center">
                                    Open auctions
                                </Typography>
                                <Typography variant="caption">
                                    Click to enter
                                </Typography>
                            </Grid>
                            <Grid item xs={12}>
                                <List>
                                    {view.tokens.map((token: string, idx: number) =>
                                        <Box key={token}>
                                            {idx !== 0 &&
                                                <Divider component="li" />
                                            }
                                            <ListItem key={token}>
                                                <RouterLink to={`/auction/${token}`}>
                                                    <ListItemIcon>
                                                        <IconButton>
                                                            <Login />
                                                        </IconButton>
                                                    </ListItemIcon>
                                                </RouterLink>
                                                <ListItemText
                                                    primary={token}
                                                />
                                            </ListItem>
                                        </Box>
                                    )}
                                </List>
                            </Grid>
                        </>
                    }
                    {owner && owner === account &&
                        <>
                            <Grid item xs={12}>
                                <Box component="form" onSubmit={initAuction}>
                                    <Grid container spacing={2}>
                                        <Grid item xs={12}>
                                            <Typography variant="h5" align="center">
                                                Initialize new auction
                                            </Typography>
                                        </Grid>
                                        <Grid item xs={12}>
                                            <TextField
                                                label="Auction Token in hex"
                                                required
                                                variant="standard"
                                                inputProps={{ maxLength: 8 }}
                                                value={tokenToInitInput}
                                                onChange={(e) => handleTokenToInitInput(e.target.value)}
                                                error={Boolean(tokenToInitError)}
                                                helperText={tokenToInitError}
                                            />
                                        </Grid>
                                        <Grid item xs={12}>
                                            <TextField
                                                label="Auction participant count"
                                                variant="standard"
                                                type="number"
                                                onChange={(e) => setTokenToInitQuantity(Number(e.target.value))}
                                                value={tokenToInitQuantity}
                                            />
                                        </Grid>
                                        <Grid item xs={12}>
                                            <Button
                                                type="submit"
                                                fullWidth
                                                variant="contained"
                                                disabled={processing || !provider || !account || !tokenToInit || Boolean(tokenToInitError) || !contractId}
                                            >
                                                Initialize
                                            </Button>
                                        </Grid>
                                    </Grid>
                                </Box>
                            </Grid>
                            <Grid item xs={12}>
                                <Box component="form" onSubmit={submitBurnAuction}>
                                    <Grid container spacing={2}>
                                        <Grid item xs={12}>
                                            <Typography variant="h5" align="center">
                                                Burn auction
                                            </Typography>
                                        </Grid>
                                        <Grid item xs={12}>
                                            <TextField
                                                label="Auction Token to Burn in hex"
                                                required
                                                variant="standard"
                                                value={tokenToBurnInput}
                                                onChange={(e) => handleTokenToBurnInput(e.target.value)}
                                                error={Boolean(tokenToBurnError)}
                                                helperText={tokenToBurnError}
                                            />
                                        </Grid>
                                        <Grid item xs={12}>
                                            <Button
                                                type="submit"
                                                fullWidth
                                                variant="contained"
                                                disabled={processing || !provider || !account || !tokenToBurn || Boolean(tokenToBurnError) || !contractId}
                                            >
                                                Burn
                                            </Button>
                                        </Grid>
                                    </Grid>
                                </Box>
                            </Grid>
                        </>
                    }
                    <Grid item xs={12}>
                        <Box component="form" onSubmit={initializeContract}>
                            <Grid container spacing={2}>
                                <Grid item xs={12}>
                                    <Typography variant="h5" align="center">
                                        Initialize new contract
                                    </Typography>
                                </Grid>
                                <Grid item xs={12}>
                                    <Button
                                        type="submit"
                                        fullWidth
                                        variant="contained"
                                        disabled={processing || !provider || !account}
                                    >
                                        Initialize
                                    </Button>
                                </Grid>
                            </Grid>
                        </Box>
                    </Grid>
                </Grid>
            </Paper>
        </Container >
    )
}