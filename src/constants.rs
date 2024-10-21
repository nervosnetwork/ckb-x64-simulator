pub const SYS_EXIT: u64 = 93;
pub const SYS_VM_VERSION: u64 = 2041;
pub const SYS_CURRENT_CYCLES: u64 = 2042;
pub const SYS_EXEC: u64 = 2043;
pub const SYS_LOAD_TRANSACTION: u64 = 2051;
pub const SYS_LOAD_SCRIPT: u64 = 2052;
pub const SYS_LOAD_TX_HASH: u64 = 2061;
pub const SYS_LOAD_SCRIPT_HASH: u64 = 2062;
pub const SYS_LOAD_CELL: u64 = 2071;
pub const SYS_LOAD_HEADER: u64 = 2072;
pub const SYS_LOAD_INPUT: u64 = 2073;
pub const SYS_LOAD_WITNESS: u64 = 2074;
pub const SYS_LOAD_CELL_BY_FIELD: u64 = 2081;
pub const SYS_LOAD_HEADER_BY_FIELD: u64 = 2082;
pub const SYS_LOAD_INPUT_BY_FIELD: u64 = 2083;
pub const SYS_LOAD_CELL_DATA_AS_CODE: u64 = 2091;
pub const SYS_LOAD_CELL_DATA: u64 = 2092;
pub const SYS_DEBUG: u64 = 2177;

// https://github.com/nervosnetwork/ckb-c-stdlib/blob/744c62e5259a5ab826e1a02ca36a811c9905f010/ckb_consts.h#L32
pub const CKB_SUCCESS: i32 = 0;
pub const CKB_INDEX_OUT_OF_BOUND: i32 = 1;
pub const CKB_ITEM_MISSING: i32 = 2;
pub const CKB_WAIT_FAILURE: i32 = 5;
pub const CKB_INVALID_FD: i32 = 6;
pub const CKB_OTHER_END_CLOSED: i32 = 7;
pub const CKB_MAX_VMS_SPAWNED: i32 = 8;
pub const CKB_MAX_FDS_CREATED: i32 = 9;

pub const SOURCE_INPUT: u64 = 1;
pub const SOURCE_OUTPUT: u64 = 2;
pub const SOURCE_CELL_DEP: u64 = 3;
pub const SOURCE_HEADER_DEP: u64 = 4;
pub const SOURCE_GROUP_INPUT: u64 = 0x0100000000000001;
pub const SOURCE_GROUP_OUTPUT: u64 = 0x0100000000000002;
pub const SOURCE_GROUP_CELL_DEP: u64 = 0x0100000000000003;
pub const SOURCE_GROUP_HEADER_DEP: u64 = 0x0100000000000004;

pub const CELL_FIELD_CAPACITY: u64 = 0;
pub const CELL_FIELD_DATA_HASH: u64 = 1;
pub const CELL_FIELD_LOCK: u64 = 2;
pub const CELL_FIELD_LOCK_HASH: u64 = 3;
pub const CELL_FIELD_TYPE: u64 = 4;
pub const CELL_FIELD_TYPE_HASH: u64 = 5;
pub const CELL_FIELD_OCCUPIED_CAPACITY: u64 = 6;

pub const HEADER_FIELD_EPOCH_NUMBER: u64 = 0;
pub const HEADER_FIELD_EPOCH_START_BLOCK_NUMBER: u64 = 1;
pub const HEADER_FIELD_EPOCH_LENGTH: u64 = 2;

pub const INPUT_FIELD_OUT_POINT: u64 = 0;
pub const INPUT_FIELD_SINCE: u64 = 1;
