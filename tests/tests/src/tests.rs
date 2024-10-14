// Include your tests here
// See https://github.com/xxuejie/ckb-native-build-sample/blob/main/tests/src/tests.rs for examples

use ckb_testtool::{
    ckb_error::Error as CKBError,
    ckb_types::{
        bytes::Bytes,
        core::{Cycle, DepType, ScriptHashType, TransactionBuilder},
        packed::{CellDep, CellInput, CellOutput},
        prelude::*,
    },
    context::Context,
};
use spawn_cmd::SpawnCmd;

pub const MAX_CYCLES: u64 = 500_000_000_000;

#[test]
fn test_exec() {
    let mut context = Context::default();
    context.add_contract_dir("../target/debug/");
    context.add_contract_dir("target/debug/");

    let out_point_exec_parent = context.deploy_cell_by_name("exec-parent");
    let out_point_exec_child = context.deploy_cell_by_name("exec-child");

    let exec_child_code_hash = context
        .cells
        .get(&out_point_exec_child)
        .map(|(_, bin)| CellOutput::calc_data_hash(bin).as_bytes().to_vec())
        .unwrap();
    println!("=== exec child code hash: {:02x?}", &exec_child_code_hash);

    let lock_script = context
        .build_script_with_hash_type(
            &out_point_exec_parent,
            ScriptHashType::Data2,
            Default::default(),
        )
        .expect("script")
        .as_builder()
        .args(
            vec![exec_child_code_hash, vec![ScriptHashType::Data2.into()]]
                .concat()
                .pack(),
        )
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
        .cell_dep(
            CellDep::new_builder()
                .out_point(out_point_exec_child)
                .dep_type(DepType::Code.into())
                .build(),
        )
        .build();

    let tx = context.complete_tx(tx);

    // run
    context
        .verify_tx(&tx, MAX_CYCLES)
        .expect("pass verification");
}

fn run_spawn(cmd: SpawnCmd, args: &[u8]) -> Result<Cycle, CKBError> {
    let mut context = Context::default();
    context.add_contract_dir("../target/debug/");
    context.add_contract_dir("target/debug/");

    let out_point_parent = context.deploy_cell_by_name("spawn-parent");
    let out_point_child = context.deploy_cell_by_name("spawn-child");

    // let exec_child_code_hash = context
    //     .cells
    //     .get(&out_point_child)
    //     .map(|(_, bin)| CellOutput::calc_data_hash(bin).as_bytes().to_vec())
    //     .unwrap();
    // println!("=== spawn child code hash: {:02x?}", exec_child_code_hash);

    let args = {
        let child_code_hash = context
            .cells
            .get(&out_point_child)
            .map(|(_, bin)| CellOutput::calc_data_hash(bin).as_bytes().to_vec())
            .unwrap();

        vec![vec![cmd.into()], child_code_hash, args.to_vec()].concat()
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
        .cell_dep(
            CellDep::new_builder()
                .out_point(out_point_child)
                .dep_type(DepType::Code.into())
                .build(),
        )
        .build();

    let tx = context.complete_tx(tx);

    // run
    context.verify_tx(&tx, MAX_CYCLES)
}

fn run_spawn_success(cmd: SpawnCmd, args: &[u8]) {
    run_spawn(cmd, args).expect("pass verification");
}

fn _run_spawn_failed(cmd: SpawnCmd, args: &[u8]) {
    let err = run_spawn(cmd, args).unwrap_err();
    println!("Spawn error: {:?}", err);
}

#[test]
fn test_spawn_base() {
    run_spawn_success(SpawnCmd::Base, &[]);
}

#[test]
fn test_spawn_ret_not0() {
    run_spawn_success(SpawnCmd::SpawnRetNot0, &[]);
}

// #[test]
// fn test_wait_ret_not0() {
//     run_spawn_failed(SpawnCmd::WaitRetNot0, &[]);
// }

#[test]
fn test_wait_invalid_pid() {
    run_spawn_success(SpawnCmd::WaitInvalidPid, &[]);
}

#[test]
fn test_spawn_empty_pipe() {
    run_spawn_success(SpawnCmd::EmptyPipe, &[]);
}

#[test]
fn test_spawn_invalid_fd() {
    run_spawn_success(SpawnCmd::SpawnInvalidFd, &[]);
}

#[test]
fn test_spawn_max_vms() {
    run_spawn_success(SpawnCmd::SpawnMaxVms, &[]);
}

#[test]
fn test_pipe_max_fds() {
    run_spawn_success(SpawnCmd::PipeMaxFds, &[]);
}

#[test]
fn test_spawn_io1() {
    run_spawn_success(SpawnCmd::BaseIO1, &[]);
}

#[test]
fn test_spawn_io2() {
    run_spawn_success(SpawnCmd::BaseIO2, &[]);
}

#[test]
fn test_spawn_io3() {
    run_spawn_success(SpawnCmd::BaseIO3, &[]);
}

#[test]
fn test_spawn_io4() {
    run_spawn_success(SpawnCmd::BaseIO4, &[]);
}

#[test]
fn test_io_read_more() {
    run_spawn_success(SpawnCmd::IOReadMore, &[]);
}

#[test]
fn test_io_write_more() {
    run_spawn_success(SpawnCmd::IOWriteMore, &[]);
}
