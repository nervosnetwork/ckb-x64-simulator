use crate::tests::MAX_CYCLES;
use ckb_testtool::{
    ckb_error::Error as CKBError,
    ckb_types::{
        bytes::Bytes,
        core::{Cycle, ScriptHashType, TransactionBuilder},
        packed::{CellInput, CellOutput},
        prelude::*,
    },
    context::Context,
};
use spawn_cmd::SpawnCasesCmd;

fn run_spawn_cases(cmd: SpawnCasesCmd, args: &[u8]) -> Result<Cycle, CKBError> {
    let mut context = Context::default();
    context.add_contract_dir("../target/debug/");
    context.add_contract_dir("target/debug/");

    let out_point_parent = context.deploy_cell_by_name("spawn-cases");

    let args = {
        vec![vec![cmd.into()], args.to_vec()].concat()
    };

    let lock_script = context
        .build_script_with_hash_type(&out_point_parent, ScriptHashType::Data2, Default::default())
        .expect("script")
        .as_builder()
        .args(args.pack())
        .build();
    let input: CellInput = CellInput::new_builder()
        .previous_output(
            context.create_cell(
                CellOutput::new_builder()
                    .capacity(1000u64.pack())
                    .lock(lock_script.clone())
                    .build(),
                Bytes::new(),
            ),
        )
        .build();

    let outputs = vec![
        CellOutput::new_builder()
            .capacity(500u64.pack())
            .lock(lock_script.clone())
            .build(),
        CellOutput::new_builder()
            .capacity(500u64.pack())
            .lock(lock_script)
            .build(),
    ];

    let outputs_data = vec![Bytes::new(); 2];

    // build transaction
    let tx = TransactionBuilder::default()
        // .set_inputs(vec![input, input3, input2])
        .set_inputs(vec![input])
        .outputs(outputs)
        .outputs_data(outputs_data.pack())
        .build();

    let tx = context.complete_tx(tx);

    // run
    context.verify_tx(&tx, MAX_CYCLES)
}

#[test]
fn check_spawn_simple_read_write() {
    let result = run_spawn_cases(SpawnCasesCmd::ReadWrite, &[]);
    let _ = result.expect("pass");
}

// There is no automated testing here, the simulator will throw an exception here.
// #[test]
// fn check_spawn_write_dead_lock() {
//     let result = run_spawn_cases(SpawnCasesCmd::WriteDeadLock, &[]);
//     assert!(result.unwrap_err().to_string().contains("deadlock"));
// }

#[test]
fn check_spawn_invalid_fd() {
    let result = run_spawn_cases(SpawnCasesCmd::InvalidFd, &[]);
    result.expect("pass");
}

//
// #[test]
// fn check_spawn_wait_dead_lock() {
//     let result = run_spawn_cases(SpawnCasesCmd::WaitDeadLock, &[]);
//     assert!(result.unwrap_err().to_string().contains("deadlock"));
// }

#[test]
fn check_spawn_read_write_with_close() {
    let result = run_spawn_cases(SpawnCasesCmd::ReadWriteWithClose, &[]);
    result.expect("pass");
}

#[test]
fn check_spawn_wait_multiple() {
    let result = run_spawn_cases(SpawnCasesCmd::WaitMultiple, &[]);
    result.expect("pass");
}

#[test]
fn check_spawn_inherited_fds() {
    let result = run_spawn_cases(SpawnCasesCmd::InheritedFds, &[]);
    result.expect("pass");
}

#[test]
fn check_spawn_inherited_fds_without_owner() {
    let result = run_spawn_cases(SpawnCasesCmd::InheritedFdsWithoutOwner, &[]);
    result.expect("pass");
}

#[test]
fn check_spawn_read_then_close() {
    let result = run_spawn_cases(SpawnCasesCmd::ReadThenClose, &[]);
    result.expect("pass");
}

#[test]
fn check_spawn_max_vms_count() {
    let result = run_spawn_cases(SpawnCasesCmd::MaxVmsCount, &[]);
    result.expect("pass");
}

#[test]
fn check_spawn_max_fds_limit() {
    let result = run_spawn_cases(SpawnCasesCmd::MaxFdsLimit, &[]);
    result.expect("pass");
}

#[test]
fn check_close_invalid_fd() {
    let result = run_spawn_cases(SpawnCasesCmd::CloseInvalidFd, &[]);
    result.expect("pass");
}

#[test]
fn check_write_closed_fd() {
    let result = run_spawn_cases(SpawnCasesCmd::WriteClosedFd, &[]);
    result.expect("pass");
}

#[test]
fn check_pid() {
    let result = run_spawn_cases(SpawnCasesCmd::CheckPID, &[]);
    result.expect("pass");
}
