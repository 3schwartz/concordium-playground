#!/bin/bash

cd ./contract

mkdir -p dist

cargo concordium build --out dist/module.wasm.v1 --schema-out dist/schema.bin

cargo concordium schema-base64 --schema dist/schema.bin --out dist/base64_schema.b64

cd ..

cargo run --manifest-path ./schema/Cargo.toml ./contract/dist/base64_schema.b64 ./frontend/src/schema.json

cargo run --manifest-path ./generators/Cargo.toml

mkdir -p logs

touch ./logs/verifier.log
touch ./logs/frontend.log

cargo run --manifest-path ./verifier/Cargo.toml &> ./logs/verifier.log &

VERIFIER_PID=$!
trap 'kill $VERIFIER_PID' ERR EXIT;

cd ./frontend

npm install

npm start &> ../logs/frontend.log