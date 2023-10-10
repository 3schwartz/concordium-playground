itest:
	cargo concordium test --out concordium-out/module.wasm.v1

utest:
	cargo test -- --nocapture

build:
	cargo concordium build --schema-embed --out dist/module.wasm.v1

schema:
	cargo concordium build --schema-template-out - > foo.txt

sequence:
	java -jar ./docs/plantuml-mit-1.2023.11.jar  ./docs/sequence