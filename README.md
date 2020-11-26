# ckb-x64-simulator

ckb-x64-simulator provides a simulator environment, which can be used to compile CKB smart contracts to native x64 environment. The result here, is that all the existing toolings on x64 environment, such as valgrind, address sanitizer, undefined behavior sanitizer, code coverage tools, etc. can be used to ensure the security of smart contracts. One day we might reach the point that RISC-V based toolings have caught up, so this simulator can be sunset, but at the moment now, it provides a good tradeoff to boost smart contract security.

While this simulator is written in pure Rust, C based APIs are exposed so it can also be linked against a C based smart contract.
