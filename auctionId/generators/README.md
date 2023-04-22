# Generate public and private key files for dApp

From root
```
cargo run --manifest-path ./generators/Cargo.toml
```

It will generate keys needed for verifier and frontend and generate files used by these.

Example output
```
...

Secret key: b1d6d3f9e992e74f2e0333387f50ac45e20b6f1212102f302ee251b6d72306bf
Public key: 34fb5a83a487bfcd0a198630324f90e5cb2e71cdb961bd4ffb2f62cdfe2b3d21
Successfully created files
```
