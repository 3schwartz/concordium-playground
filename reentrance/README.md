# Reentrance example

This project contains example of contracts with reentrance exploit and contracts which mitigates the issue.

## Commands to build and run the project

```
# Build schema
cargo concordium build --schema-template-out -

# Build module
cargo concordium build --schema-embed --out dist/module.wasm.v1

# Deploy module
concordium-client module deploy dist/module.wasm.v1 --sender <ACCOUNT> --name reentrance --grpc-port 20000 --grpc-ip node.testnet.concordium.com

# Initialize contracts
concordium-client contract init reentrance --sender <ACCOUNT> --contract reentrance --name reentrance --energy 2000 --grpc-port 20000 --grpc-ip node.testnet.concordium.com
concordium-client contract init reentrance --sender <ACCOUNT> --contract attacker --name attacker --energy 2000 --grpc-port 20000 --grpc-ip node.testnet.concordium.com

# Invoke or update contracts
concordium-client contract update reentrance --entrypoint deposit --energy 3000 --sender <ACCOUNT> --amount 2 --grpc-port 20000 --grpc-ip node.testnet.concordium.com
concordium-client contract invoke reentrance --entrypoint view --grpc-port 20000 --grpc-ip node.testnet.concordium.com
```

## Energy used

Energy used by contracts in project. Energy comes from the Concordium integration test framework.


| Contract                                  | Energy    |
|-------------------------------------------|-----------|
| Reentrance                                | 4793      |
| Reentrance readonly                       | 4792      |
| Reentrance checks effects interactions    | 4793      |
| Reentrance mutex                          | 4793      |