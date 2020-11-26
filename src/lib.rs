pub mod constants;

#[macro_use]
extern crate lazy_static;

use ckb_standalone_debugger::transaction::{MockTransaction, ReprMockTransaction};
use ckb_types::{
    bytes::Bytes,
    core::{cell::CellMetaBuilder, Capacity, HeaderView},
    packed::{self, Byte32, CellInput, CellOutput, Script},
    prelude::{Entity, Unpack},
};
use constants::{
    CELL_FIELD_CAPACITY, CELL_FIELD_DATA_HASH, CELL_FIELD_LOCK, CELL_FIELD_LOCK_HASH,
    CELL_FIELD_OCCUPIED_CAPACITY, CELL_FIELD_TYPE, CELL_FIELD_TYPE_HASH, CKB_INDEX_OUT_OF_BOUND,
    CKB_ITEM_MISSING, CKB_SUCCESS, HEADER_FIELD_EPOCH_LENGTH, HEADER_FIELD_EPOCH_NUMBER,
    HEADER_FIELD_EPOCH_START_BLOCK_NUMBER, INPUT_FIELD_OUT_POINT, INPUT_FIELD_SINCE,
    SOURCE_CELL_DEP, SOURCE_GROUP_CELL_DEP, SOURCE_GROUP_HEADER_DEP, SOURCE_GROUP_INPUT,
    SOURCE_GROUP_OUTPUT, SOURCE_HEADER_DEP, SOURCE_INPUT, SOURCE_OUTPUT,
};
use serde_derive::{Deserialize, Serialize};
use std::ffi::CStr;
use std::os::raw::{c_char, c_int, c_void};

#[derive(Clone, Serialize, Deserialize)]
pub struct RunningSetup {
    pub is_lock_script: bool,
    pub is_output: bool,
    pub script_index: u64,
}

lazy_static! {
    static ref TRANSACTION: MockTransaction = {
        let tx_filename = std::env::var("CKB_TX_FILE").expect("environment variable");
        let tx_content = std::fs::read_to_string(tx_filename).expect("read tx file");
        let repr_mock_tx: ReprMockTransaction =
            serde_json::from_str(&tx_content).expect("parse tx file");
        let mock_tx: MockTransaction = repr_mock_tx.into();
        mock_tx
    };
    static ref SETUP: RunningSetup = {
        let setup_filename = std::env::var("CKB_RUNNING_SETUP").expect("environment variable");
        let setup_content = std::fs::read_to_string(setup_filename).expect("read setup file");
        serde_json::from_str(&setup_content).expect("parse setup file")
    };
}

#[no_mangle]
pub extern "C" fn ckb_exit(code: i8) -> i32 {
    std::process::exit(code.into());
}

#[no_mangle]
pub extern "C" fn ckb_load_tx_hash(ptr: *mut c_void, len: *mut u64, offset: u64) -> c_int {
    let view = TRANSACTION.tx.clone().into_view();
    store_data(ptr, len, offset, view.hash().as_slice());
    CKB_SUCCESS
}

#[no_mangle]
pub extern "C" fn ckb_load_transaction(ptr: *mut c_void, len: *mut u64, offset: u64) -> c_int {
    store_data(ptr, len, offset, TRANSACTION.tx.as_slice());
    CKB_SUCCESS
}

#[no_mangle]
pub extern "C" fn ckb_load_script_hash(ptr: *mut c_void, len: *mut u64, offset: u64) -> c_int {
    let hash = fetch_current_script().calc_script_hash();
    store_data(ptr, len, offset, hash.as_slice());
    CKB_SUCCESS
}

#[no_mangle]
pub extern "C" fn ckb_load_script(ptr: *mut c_void, len: *mut u64, offset: u64) -> c_int {
    store_data(ptr, len, offset, fetch_current_script().as_slice());
    CKB_SUCCESS
}

#[no_mangle]
pub extern "C" fn ckb_debug(s: *const c_char) {
    let message = unsafe { CStr::from_ptr(s) }.to_str().expect("UTF8 error!");
    println!("Debug message: {}", message);
}

#[no_mangle]
pub extern "C" fn ckb_load_cell(
    ptr: *mut c_void,
    len: *mut u64,
    offset: u64,
    index: u64,
    source: u64,
) -> c_int {
    let (cell, _) = match fetch_cell(index, source) {
        Ok(cell) => cell,
        Err(code) => return code,
    };
    store_data(ptr, len, offset, cell.as_slice());
    CKB_SUCCESS
}

#[no_mangle]
pub extern "C" fn ckb_load_input(
    ptr: *mut c_void,
    len: *mut u64,
    offset: u64,
    index: u64,
    source: u64,
) -> c_int {
    let input = match fetch_input(index, source) {
        Ok(input) => input,
        Err(code) => return code,
    };
    store_data(ptr, len, offset, input.as_slice());
    CKB_SUCCESS
}

#[no_mangle]
pub extern "C" fn ckb_load_header(
    ptr: *mut c_void,
    len: *mut u64,
    offset: u64,
    index: u64,
    source: u64,
) -> c_int {
    let header = match fetch_header(index, source) {
        Ok(input) => input,
        Err(code) => return code,
    };
    store_data(ptr, len, offset, header.data().as_slice());
    CKB_SUCCESS
}

#[no_mangle]
pub extern "C" fn ckb_load_witness(
    ptr: *mut c_void,
    len: *mut u64,
    offset: u64,
    index: u64,
    source: u64,
) -> c_int {
    let witness = match fetch_witness(index, source) {
        Some(witness) => witness,
        None => return CKB_INDEX_OUT_OF_BOUND,
    };
    store_data(ptr, len, offset, &witness.raw_data());
    CKB_SUCCESS
}

#[no_mangle]
pub extern "C" fn ckb_load_cell_by_field(
    ptr: *mut c_void,
    len: *mut u64,
    offset: u64,
    index: u64,
    source: u64,
    field: u64,
) -> c_int {
    let (cell, cell_data) = match fetch_cell(index, source) {
        Ok(cell) => cell,
        Err(code) => return code,
    };
    let cell_meta = CellMetaBuilder::from_cell_output(cell.clone(), cell_data.clone()).build();
    match field {
        CELL_FIELD_CAPACITY => {
            let capacity: Capacity = cell.capacity().unpack();
            let data = capacity.as_u64().to_le_bytes();
            store_data(ptr, len, offset, &data[..]);
        }
        CELL_FIELD_DATA_HASH => {
            let hash = CellOutput::calc_data_hash(&cell_data);
            store_data(ptr, len, offset, hash.as_slice());
        }
        CELL_FIELD_OCCUPIED_CAPACITY => {
            let data = cell_meta
                .occupied_capacity()
                .expect("capacity error")
                .as_u64()
                .to_le_bytes();
            store_data(ptr, len, offset, &data[..]);
        }
        CELL_FIELD_LOCK => {
            let lock = cell.lock();
            store_data(ptr, len, offset, lock.as_slice());
        }
        CELL_FIELD_LOCK_HASH => {
            let hash = cell.calc_lock_hash();
            store_data(ptr, len, offset, &hash.as_bytes());
        }
        CELL_FIELD_TYPE => match cell.type_().to_opt() {
            Some(type_) => {
                store_data(ptr, len, offset, type_.as_slice());
            }
            None => {
                return CKB_ITEM_MISSING;
            }
        },
        CELL_FIELD_TYPE_HASH => match cell.type_().to_opt() {
            Some(type_) => {
                let hash = type_.calc_script_hash();
                store_data(ptr, len, offset, &hash.as_bytes());
            }
            None => {
                return CKB_ITEM_MISSING;
            }
        },
        _ => panic!("Invalid field: {}", field),
    };
    CKB_SUCCESS
}

#[no_mangle]
pub extern "C" fn ckb_load_header_by_field(
    ptr: *mut c_void,
    len: *mut u64,
    offset: u64,
    index: u64,
    source: u64,
    field: u64,
) -> c_int {
    let header = match fetch_header(index, source) {
        Ok(input) => input,
        Err(code) => return code,
    };
    let epoch = header.epoch();
    let value = match field {
        HEADER_FIELD_EPOCH_NUMBER => epoch.number(),
        HEADER_FIELD_EPOCH_START_BLOCK_NUMBER => header
            .number()
            .checked_sub(epoch.index())
            .expect("Overflow!"),
        HEADER_FIELD_EPOCH_LENGTH => epoch.length(),
        _ => panic!("Invalid field: {}", field),
    };
    let data = value.to_le_bytes();
    store_data(ptr, len, offset, &data[..]);
    CKB_SUCCESS
}

#[no_mangle]
pub extern "C" fn ckb_load_input_by_field(
    ptr: *mut c_void,
    len: *mut u64,
    offset: u64,
    index: u64,
    source: u64,
    field: u64,
) -> c_int {
    let input = match fetch_input(index, source) {
        Ok(input) => input,
        Err(code) => return code,
    };
    match field {
        INPUT_FIELD_OUT_POINT => {
            store_data(ptr, len, offset, input.previous_output().as_slice());
        }
        INPUT_FIELD_SINCE => {
            let since: u64 = input.since().unpack();
            let data = since.to_le_bytes();
            store_data(ptr, len, offset, &data[..]);
        }
        _ => panic!("Invalid field: {}", field),
    };
    CKB_SUCCESS
}

#[no_mangle]
pub extern "C" fn ckb_load_cell_data(
    ptr: *mut c_void,
    len: *mut u64,
    offset: u64,
    index: u64,
    source: u64,
) -> c_int {
    let (_, cell_data) = match fetch_cell(index, source) {
        Ok(cell) => cell,
        Err(code) => return code,
    };
    store_data(ptr, len, offset, &cell_data);
    CKB_SUCCESS
}

fn fetch_cell(index: u64, source: u64) -> Result<(CellOutput, Bytes), c_int> {
    match source {
        SOURCE_INPUT => TRANSACTION
            .mock_info
            .inputs
            .get(index as usize)
            .ok_or(CKB_INDEX_OUT_OF_BOUND)
            .map(|input| (input.output.clone(), input.data.clone())),
        SOURCE_OUTPUT => TRANSACTION
            .tx
            .raw()
            .outputs()
            .get(index as usize)
            .ok_or(CKB_INDEX_OUT_OF_BOUND)
            .map(|output| {
                (
                    output,
                    TRANSACTION
                        .tx
                        .raw()
                        .outputs_data()
                        .get(index as usize)
                        .expect("cell data mismatch")
                        .unpack(),
                )
            }),
        SOURCE_CELL_DEP => TRANSACTION
            .mock_info
            .cell_deps
            .get(index as usize)
            .ok_or(CKB_INDEX_OUT_OF_BOUND)
            .map(|cell_dep| (cell_dep.output.clone(), cell_dep.data.clone())),
        SOURCE_HEADER_DEP => Err(CKB_INDEX_OUT_OF_BOUND),
        SOURCE_GROUP_INPUT => {
            let (indices, _) = fetch_group_indices();
            indices
                .get(index as usize)
                .ok_or(CKB_INDEX_OUT_OF_BOUND)
                .and_then(|actual_index| {
                    TRANSACTION
                        .mock_info
                        .inputs
                        .get(*actual_index)
                        .ok_or(CKB_INDEX_OUT_OF_BOUND)
                        .map(|input| (input.output.clone(), input.data.clone()))
                })
        }
        SOURCE_GROUP_OUTPUT => {
            let (_, indices) = fetch_group_indices();
            indices
                .get(index as usize)
                .ok_or(CKB_INDEX_OUT_OF_BOUND)
                .and_then(|actual_index| {
                    TRANSACTION
                        .tx
                        .raw()
                        .outputs()
                        .get(*actual_index)
                        .ok_or(CKB_INDEX_OUT_OF_BOUND)
                        .map(|output| {
                            (
                                output,
                                TRANSACTION
                                    .tx
                                    .raw()
                                    .outputs_data()
                                    .get(index as usize)
                                    .expect("cell data mismatch")
                                    .unpack(),
                            )
                        })
                })
        }
        SOURCE_GROUP_CELL_DEP => Err(CKB_INDEX_OUT_OF_BOUND),
        SOURCE_GROUP_HEADER_DEP => Err(CKB_INDEX_OUT_OF_BOUND),
        _ => panic!("Invalid source: {}", source),
    }
}

fn fetch_input(index: u64, source: u64) -> Result<CellInput, c_int> {
    match source {
        SOURCE_INPUT => TRANSACTION
            .tx
            .raw()
            .inputs()
            .get(index as usize)
            .ok_or(CKB_INDEX_OUT_OF_BOUND),
        SOURCE_OUTPUT => Err(CKB_INDEX_OUT_OF_BOUND),
        SOURCE_CELL_DEP => Err(CKB_INDEX_OUT_OF_BOUND),
        SOURCE_HEADER_DEP => Err(CKB_INDEX_OUT_OF_BOUND),
        SOURCE_GROUP_INPUT => {
            let (indices, _) = fetch_group_indices();
            indices
                .get(index as usize)
                .ok_or(CKB_INDEX_OUT_OF_BOUND)
                .and_then(|actual_index| {
                    TRANSACTION
                        .tx
                        .raw()
                        .inputs()
                        .get(*actual_index)
                        .ok_or(CKB_INDEX_OUT_OF_BOUND)
                })
        }
        SOURCE_GROUP_OUTPUT => Err(CKB_INDEX_OUT_OF_BOUND),
        SOURCE_GROUP_CELL_DEP => Err(CKB_INDEX_OUT_OF_BOUND),
        SOURCE_GROUP_HEADER_DEP => Err(CKB_INDEX_OUT_OF_BOUND),
        _ => panic!("Invalid source: {}", source),
    }
}

fn find_header(hash: Byte32) -> Option<HeaderView> {
    TRANSACTION
        .mock_info
        .header_deps
        .iter()
        .find(|header| header.hash() == hash)
        .cloned()
}

fn fetch_header(index: u64, source: u64) -> Result<HeaderView, c_int> {
    match source {
        SOURCE_INPUT => TRANSACTION
            .mock_info
            .inputs
            .get(index as usize)
            .and_then(|input| input.header.as_ref().cloned())
            .ok_or(CKB_INDEX_OUT_OF_BOUND)
            .and_then(|header_hash| find_header(header_hash).ok_or(CKB_ITEM_MISSING)),
        SOURCE_OUTPUT => Err(CKB_INDEX_OUT_OF_BOUND),
        SOURCE_CELL_DEP => TRANSACTION
            .mock_info
            .cell_deps
            .get(index as usize)
            .and_then(|cell_dep| cell_dep.header.as_ref().cloned())
            .ok_or(CKB_INDEX_OUT_OF_BOUND)
            .and_then(|header_hash| find_header(header_hash).ok_or(CKB_ITEM_MISSING)),
        SOURCE_HEADER_DEP => TRANSACTION
            .mock_info
            .header_deps
            .get(index as usize)
            .cloned()
            .ok_or(CKB_INDEX_OUT_OF_BOUND),
        SOURCE_GROUP_INPUT => {
            let (indices, _) = fetch_group_indices();
            indices
                .get(index as usize)
                .ok_or(CKB_INDEX_OUT_OF_BOUND)
                .and_then(|actual_index| {
                    TRANSACTION
                        .mock_info
                        .inputs
                        .get(*actual_index)
                        .and_then(|input| input.header.as_ref().cloned())
                        .ok_or(CKB_INDEX_OUT_OF_BOUND)
                        .and_then(|header_hash| find_header(header_hash).ok_or(CKB_ITEM_MISSING))
                })
        }
        SOURCE_GROUP_OUTPUT => Err(CKB_INDEX_OUT_OF_BOUND),
        SOURCE_GROUP_CELL_DEP => Err(CKB_INDEX_OUT_OF_BOUND),
        SOURCE_GROUP_HEADER_DEP => Err(CKB_INDEX_OUT_OF_BOUND),
        _ => panic!("Invalid source: {}", source),
    }
}

fn fetch_witness(index: u64, source: u64) -> Option<packed::Bytes> {
    match source {
        SOURCE_INPUT => TRANSACTION.tx.witnesses().get(index as usize),
        SOURCE_OUTPUT => TRANSACTION.tx.witnesses().get(index as usize),
        SOURCE_GROUP_INPUT => {
            let (indices, _) = fetch_group_indices();
            indices
                .get(index as usize)
                .and_then(|actual_index| TRANSACTION.tx.witnesses().get(*actual_index))
        }
        SOURCE_GROUP_OUTPUT => {
            let (_, indices) = fetch_group_indices();
            indices
                .get(index as usize)
                .and_then(|actual_index| TRANSACTION.tx.witnesses().get(*actual_index))
        }
        SOURCE_CELL_DEP => None,
        SOURCE_HEADER_DEP => None,
        SOURCE_GROUP_CELL_DEP => None,
        SOURCE_GROUP_HEADER_DEP => None,
        _ => panic!("Invalid source: {}", source),
    }
}

fn fetch_group_indices() -> (Vec<usize>, Vec<usize>) {
    let mut input_indices: Vec<usize> = vec![];
    let mut output_indices: Vec<usize> = vec![];
    let current_script = fetch_current_script();

    for (i, input) in TRANSACTION.mock_info.inputs.iter().enumerate() {
        if SETUP.is_lock_script {
            if input.output.lock() == current_script {
                input_indices.push(i);
            }
        } else if let Some(t) = input.output.type_().to_opt() {
            if t == current_script {
                input_indices.push(i);
            }
        }
    }
    for (i, output) in TRANSACTION.tx.raw().outputs().into_iter().enumerate() {
        if let Some(t) = output.type_().to_opt() {
            if t == current_script {
                output_indices.push(i);
            }
        }
    }
    (input_indices, output_indices)
}

fn fetch_current_script() -> Script {
    let cell = if SETUP.is_output {
        TRANSACTION
            .tx
            .raw()
            .outputs()
            .get(SETUP.script_index as usize)
            .expect("running script index out of bound!")
    } else {
        TRANSACTION
            .mock_info
            .inputs
            .get(SETUP.script_index as usize)
            .expect("running script index out of bound!")
            .output
            .clone()
    };
    if SETUP.is_lock_script {
        cell.lock()
    } else {
        cell.type_().to_opt().unwrap()
    }
}

fn store_data(ptr: *mut c_void, len: *mut u64, offset: u64, data: &[u8]) {
    let size_ptr = unsafe { len.as_mut().expect("casting pointer") };
    let size = *size_ptr;
    let buffer = unsafe {
        let ptr = (ptr as *mut u8).as_mut().expect("casting pointer");
        std::slice::from_raw_parts_mut(ptr, size as usize)
    };
    let data_len = data.len() as u64;
    let offset = std::cmp::min(data_len, offset);
    let full_size = data_len - offset;
    let real_size = std::cmp::min(size, full_size);
    *size_ptr = full_size;
    buffer.copy_from_slice(&data[offset as usize..(offset + real_size) as usize]);
}
