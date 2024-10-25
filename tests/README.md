# test-workspace

This project is generated using [ckb-script-templates](https://github.com/cryptape/ckb-script-templates) and is mainly intended to demonstrate how to debug contracts using the native simulator.

## Contracts
* exec-parent\exec-child
* spawn-parent\spawn-child : Support will be added once new spawn is completed

## Native Simulator Debugging
First, compile the project by running `make build-simulator`. Then, enable the `simulator` feature in `tests`. For convenience, you can also set `default = [ "simulator" ]` directly in `tests/Cargo.toml`.

A test case called `test_exec` is provided in `tests/src/tests.rs`. After enabling the `simulator`, you can use native simulator debugging and set breakpoints in the IDE to debug the contract.
