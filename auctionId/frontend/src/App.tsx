import { WalletApi } from '@concordium/browser-wallet-api-helpers';
import { useState } from 'react';
import { AppBar, Container, Toolbar, Box, Dialog, Alert, Typography, IconButton, useTheme } from '@mui/material';
import { WalletConnect } from './components/WalletConnect';
import { Overview } from './components/Overview';
import { BrowserRouter, Route, Routes, Link as RouterLink } from 'react-router-dom';
import { Home } from '@mui/icons-material';
import { Auction } from './components/Auction';

function App() {
  const theme = useTheme();

  const [account, setAccount] = useState<string>();
  const [isConnected, setIsConnected] = useState<boolean>(false);
  const [provider, setProvider] = useState<WalletApi>();
  const [error, setError] = useState<string>();

  const [contractId, setContractId] = useState<bigint>();

  return (
    <BrowserRouter>
      <header>
        <AppBar position="static">
          <Container maxWidth="xl" sx={{ height: "100%" }}>
            <Toolbar disableGutters>
              <RouterLink to="/">
                <IconButton
                  sx={{
                    backgroundColor: theme.palette.secondary.main,
                    '&:hover': {
                      backgroundColor: theme.palette.secondary.dark
                    }
                  }}>
                  <Home />
                </IconButton>
              </RouterLink>
              <Typography variant='h6' noWrap component="a"
                sx={{
                  ml: 2,
                  display: "flex",
                  fontWeight: 700,
                  letterSpacing: ".3rem",

                }}>
                Contract: {contractId ? contractId.toString(): ""}
              </Typography>
              <Box sx={{ flexGrow: 1, display: "flex", flexDirection: "row-reverse" }}>
                <WalletConnect
                  account={account}
                  setAccount={setAccount}
                  isConnecting={isConnected}
                  setIsConnected={setIsConnected}
                  provider={provider}
                  setProvider={setProvider}
                />
              </Box>
            </Toolbar>
          </Container>
        </AppBar>
      </header>
      <Container component="main">
        <Toolbar />
        <Typography component="h2" variant="h2" align="center">
          Auction
        </Typography>
        <Routes>
          <Route path="/" element={
            <Overview
              provider={provider}
              account={account}
              contractId={contractId}
              setError={setError}
              setContractId={setContractId}
            />
          } />
          <Route path="/auction/:auctionId" element={
            <Auction
              provider={provider}
              account={account}
              contractId={contractId}
              setError={setError}
            />
          } />
        </Routes>
        <Dialog
          open={Boolean(error)}
          onClose={() => setError(undefined)}>
          <Alert severity='error'>
            <pre>
              <code style={{ whiteSpace: 'pre-wrap' }}>
                {error}
              </code>
            </pre>
          </Alert>
        </Dialog>
      </Container>
    </BrowserRouter>
  );
}

export default App;
