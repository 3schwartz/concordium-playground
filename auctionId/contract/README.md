# Builds

```
cargo concordium build --out dist/module.wasm.v1 --schema-out dist/schema.bin

cargo concordium schema-base64 --schema dist/schema.bin --out dist/base64_schema.b64

```

# Deploy and interact

```
concordium-client module deploy dist/module.wasm.v1 --sender test-init --name dino_auction --grpc-port 20000 --grpc-ip node.testnet.concordium.com

concordium-client contract init dino_auction --sender test-init --contract dino_auction --name dino_auction --parameter-json input/init.json --schema dist/schema.bin --energy 5000 --grpc-port 20000 --grpc-ip node.testnet.concordium.com

concordium-client contract invoke dino_auction --entrypoint view --schema ./dist/schema.bin --grpc-port 20000 --grpc-ip node.testnet.concordium.com

concordium-client contract invoke dino_auction --entrypoint get_owner --schema ./dist/schema.bin --grpc-port 20000 --grpc-ip node.testnet.concordium.com

concordium-client contract update dino_auction --entrypoint init_auction --parameter-json ./input/init_auction.json --schema ./dist/schema.bin --sender test-init --energy 6000 --grpc-port 20000 --grpc-ip node.testnet.concordium.com

concordium-client contract invoke dino_auction --entrypoint balanceOf --parameter-json ./input/balanceOf.json --schema ./dist/schema.bin --grpc-port 20000 --grpc-ip node.testnet.concordium.com

concordium-client contract update dino_auction --entrypoint mint --parameter-json ./input/mint.json --schema ./dist/schema.bin --sender test-init --energy 6000 --grpc-port 20000 --grpc-ip node.testnet.concordium.com
```
