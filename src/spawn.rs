use crate::{
    constants::{
        CKB_INVALID_FD, CKB_MAX_FDS_CREATED, CKB_MAX_VMS_SPAWNED, CKB_OTHER_END_CLOSED,
        CKB_SUCCESS, CKB_WAIT_FAILURE,
    },
    get_cur_tx, get_cur_tx_mut,
    global_data::GlobalData,
    simulator_context::SimContext,
    utils,
    utils::{Fd, ProcID},
};
use std::os::raw::{c_int, c_void};

const MAX_FDS: usize = 64;

#[repr(C)]
#[derive(Clone)]
pub struct SpawnArgs {
    /// argc contains the number of arguments passed to the program.
    pub argc: u64,
    /// argv is a one-dimensional array of strings.
    pub argv: *const *const i8,
    /// a pointer used to save the process_id of the child process.
    pub process_id: *mut u64,
    /// an array representing the file descriptors passed to the child process. It must end with zero.
    pub inherited_fds: *const u64,
}

#[no_mangle]
pub extern "C" fn ckb_spawn_cell(
    code_hash: *const u8,
    hash_type: u8,
    offset: u32,
    length: u32,
    argc: i32,
    argv: *const *const u8,
    inherited_fds: *const u64,
    pid: *mut u64,
) -> c_int {
    // check fd:
    let inherited_fds = get_fds(inherited_fds);
    for it in &inherited_fds {
        if let Err(err) = CheckSpawn::Def.check(it) {
            return err;
        }
    }
    if get_cur_tx!().max_proc_spawned() {
        return CKB_MAX_VMS_SPAWNED;
    }

    let ckb_sim = utils::CkbNativeSimulator::new_by_hash(code_hash, hash_type, offset, length);
    let args = utils::to_vec_args(argc, argv as *const *const i8);
    let new_id = get_cur_tx_mut!().start_process(&inherited_fds, move |sim_id, pid| {
        ckb_sim.update_script_info(sim_id, pid);
        ckb_sim.ckb_std_main(args)
    });

    let event = get_cur_tx!().get_event();
    event.wait();

    unsafe { *({ pid }) = new_id.into() };
    CKB_SUCCESS
}

#[no_mangle]
pub extern "C" fn ckb_wait(pid: u64, code: *mut i8) -> c_int {
    let pid: ProcID = pid.into();
    if !get_cur_tx!().has_proc(&pid) {
        return CKB_WAIT_FAILURE;
    }
    let join_handle = get_cur_tx_mut!().exit(&pid);

    let c = if let Some(j) = join_handle {
        j.join().unwrap()
    } else {
        return CKB_WAIT_FAILURE;
    };
    unsafe { *({ code }) = c };
    CKB_SUCCESS
}

#[no_mangle]
pub extern "C" fn ckb_process_id() -> u64 {
    SimContext::pid().into()
}

#[no_mangle]
pub extern "C" fn ckb_pipe(fds: *mut u64) -> c_int {
    if get_cur_tx!().len_pipe() >= MAX_FDS {
        return CKB_MAX_FDS_CREATED;
    }

    let out = get_cur_tx_mut!().new_pipe();
    copy_fds(&[out.0, out.1], fds);
    CKB_SUCCESS
}

#[no_mangle]
pub extern "C" fn ckb_read(fd: u64, buf: *mut c_void, length: *mut usize) -> c_int {
    let fd: Fd = fd.into();

    // Check
    if let Err(e) = CheckSpawn::Read.check(&fd) {
        return e;
    }

    // wait read
    let event = get_cur_tx_mut!().wait_read(fd.clone(), unsafe { *({ length }) });
    event.wait();

    let data = get_cur_tx_mut!().read_cache(&fd);

    if !data.is_empty() {
        unsafe { std::ptr::copy_nonoverlapping(data.as_ptr(), buf as *mut u8, data.len()) };
    }
    unsafe {
        *({ length }) = data.len();
    }

    CKB_SUCCESS
}

#[no_mangle]
pub extern "C" fn ckb_write(fd: u64, buf: *const c_void, length: *mut usize) -> c_int {
    let fd: Fd = fd.into();

    if let Err(e) = CheckSpawn::Write.check(&fd) {
        return e;
    }

    let buf = unsafe {
        let length = utils::to_usize(length);
        std::slice::from_raw_parts(buf as *const u8, length)
    }
    .to_vec();
    let event = get_cur_tx_mut!().wait_write(fd, &buf);
    event.wait();

    CKB_SUCCESS
}

#[no_mangle]
pub extern "C" fn ckb_inherited_fds(fds: *mut u64, length: *mut usize) -> c_int {
    let out_fds = get_cur_tx!().inherited_fds();
    let len = out_fds.len().min(utils::to_usize(length));

    copy_fds(&out_fds[0..len], fds);
    unsafe { *({ length }) = len };
    CKB_SUCCESS
}

#[no_mangle]
pub extern "C" fn ckb_close(fd: u64) -> c_int {
    let fd = fd.into();
    let event = get_cur_tx_mut!().close_pipe(fd);
    if let Ok(event) = event {
        event.wait();
        CKB_SUCCESS
    } else {
        CKB_INVALID_FD
    }
}

#[no_mangle]
pub extern "C" fn ckb_load_block_extension(
    _addr: *mut c_void,
    _len: *mut u64,
    _offset: usize,
    _index: usize,
    _source: usize,
) -> c_int {
    panic!("unsupport");
}

fn copy_fds(in_fd: &[Fd], out_fd: *mut u64) {
    let mut out_fd = out_fd;
    for fd in in_fd {
        unsafe {
            *out_fd = fd.clone().into();
            out_fd = out_fd.add(1);
        }
    }
}

fn get_fds(fds: *const u64) -> Vec<Fd> {
    unsafe {
        let mut buf = Vec::new();
        let mut fds_ptr = fds;
        while *fds_ptr != 0 {
            buf.push((*fds_ptr).into());
            fds_ptr = fds_ptr.add(1);
        }
        buf
    }
}

enum CheckSpawn {
    Def,
    Read,
    Write,
}
impl CheckSpawn {
    fn check(self, fd: &Fd) -> Result<(), c_int> {
        match self {
            Self::Def => (),
            Self::Read => {
                if !fd.is_read() {
                    return Err(CKB_INVALID_FD);
                }
            }
            Self::Write => {
                if fd.is_read() {
                    return Err(CKB_INVALID_FD);
                }
            }
        }

        let g = GlobalData::locked();
        let tx_ctx = g.get_tx(&SimContext::ctx_id());
        if !tx_ctx.has_fd(fd) {
            return Err(CKB_INVALID_FD);
        }
        if !tx_ctx.chech_other_fd(fd) {
            return Err(CKB_OTHER_END_CLOSED);
        }
        Ok(())
    }
}
