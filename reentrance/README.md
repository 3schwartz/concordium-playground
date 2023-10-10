```
cargo concordium build --schema-template-out -

cargo concordium build --schema-embed --out dist/module.wasm.v1

concordium-client module deploy dist/module.wasm.v1 --sender darth --name reentrance2 --grpc-port 20000 --grpc-ip node.testnet.concordium.com

concordium-client contract init reentrance2 --sender darth --contract reentrance --name reentrance2 --energy 2000 --grpc-port 20000 --grpc-ip node.testnet.concordium.com

concordium-client contract init reentrance2 --sender darth --contract attacker --name attacker2 --energy 2000 --grpc-port 20000 --grpc-ip node.testnet.concordium.com

concordium-client contract update reentrance2 --entrypoint deposit --energy 3000 --sender darth --amount 2 --grpc-port 20000 --grpc-ip node.testnet.concordium.com


concordium-client contract invoke reentrance2  --entrypoint view --grpc-port 20000 --grpc-ip node.testnet.concordium.com
```


# Energy used

| Contract                                  | Energy    |
|-------------------------------------------|-----------|
| Reentrance                                | 4793      |
| Reentrance readonly                       | 4792      |
| Reentrance checks effects interactions    | 4793      |
| Reentrance mutex                          | 4793      |