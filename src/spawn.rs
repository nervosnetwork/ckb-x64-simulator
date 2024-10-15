use crate::{
    get_cur_tx, get_cur_tx_mut, get_cur_vm,
    global_data::{GlobalData, VmID},
    utils,
    simulator_context::{Fd, TxContext, VMInfo},
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
    if get_cur_tx!().max_vms_spawned() {
        return 8; //MAX_VMS_SPAWNED
    }

    let ckb_sim = utils::CkbNativeSimulator::new_by_hash(code_hash, hash_type, offset, length);
    let new_id = new_vm_id(&inherited_fds);
    let jh = ckb_sim.ckb_std_main_async(argc, argv, &new_id);

    get_cur_tx_mut!().vm_mut_info(&new_id).set_join(jh);
    let event = get_cur_vm!().get_event_by_pid(&new_id);
    event.wait();

    unsafe { *({ pid }) = new_id.into() };
    0
}

#[no_mangle]
pub extern "C" fn ckb_wait(pid: u64, code: *mut i8) -> c_int {
    let pid: VmID = pid.into();
    if !get_cur_tx!().has_vm(&pid) {
        return 5; // WaitFailure
    }
    let jh = get_cur_tx_mut!().vm_mut_info(&pid).wait_exit();

    let c = if let Some(j) = jh {
        j.join().unwrap()
    } else {
        0
    };
    unsafe { *({ code }) = c };
    0
}

#[no_mangle]
pub extern "C" fn ckb_process_id() -> u64 {
    VMInfo::ctx_id().into()
}

#[no_mangle]
pub extern "C" fn ckb_pipe(fds: *mut u64) -> c_int {
    if get_cur_tx!().len_pipe() >= MAX_FDS {
        return 9; // MAX_FDS_CREATED
    }

    let out = get_cur_tx_mut!().new_pipe();
    copy_fds(&[out.0, out.1], fds);
    0
}

#[no_mangle]
pub extern "C" fn ckb_read(fd: u64, buf: *mut c_void, length: *mut usize) -> c_int {
    let fd: Fd = fd.into();

    // Check
    if let Err(e) = CheckSpawn::Read.check(&fd) {
        return e;
    }

    let has_data = get_cur_tx!().has_data(&fd);
    if !has_data {
        get_cur_vm!().notify(Some(&fd));
        let event = get_cur_vm!().wait(Some(&fd));
        event.wait();
    }

    let mut readed_len = 0;
    let mut buf_len = unsafe { *({ length }) };
    let mut buf = buf;
    while buf_len != 0 {
        if !get_cur_tx!().chech_other_fd(&fd) {
            break;
        }

        let (data, cache_size) = get_cur_tx_mut!().read_data(&fd, buf_len);
        if !data.is_empty() {
            unsafe { std::ptr::copy_nonoverlapping(data.as_ptr(), buf as *mut u8, data.len()) };
        }
        readed_len += data.len();
        buf_len -= data.len();
        unsafe {
            buf = buf.add(data.len());
        };
        if !has_data {
            break;
        }

        if cache_size == 0 {
            get_cur_vm!().notify(Some(&fd));
            let event = get_cur_vm!().wait(Some(&fd));
            event.wait();
            break;
        }
    }

    utils::set_usize(length, readed_len);

    0
}

#[no_mangle]
pub extern "C" fn ckb_write(fd: u64, buf: *const c_void, length: *mut usize) -> c_int {
    let fd: Fd = fd.into();

    if let Err(e) = CheckSpawn::Write.check(&fd) {
        return e;
    }
    let has_data = get_cur_tx_mut!().has_data(&fd);

    if has_data {
        get_cur_vm!().notify(Some(&fd));
        let event = get_cur_vm!().wait(Some(&fd));
        event.wait();
    }

    if buf.is_null() || utils::to_usize(length) == 0 {
        utils::set_usize(length, 0);
        return 0;
    }
    let buf = unsafe {
        let length = utils::to_usize(length);
        std::slice::from_raw_parts(buf as *const u8, length)
    }
    .to_vec();
    get_cur_tx_mut!().write_data(&fd, &buf);

    // if !has_data {
    get_cur_vm!().notify(Some(&fd));
    let event = get_cur_vm!().wait(Some(&fd));
    event.wait();
    // }

    0
}

#[no_mangle]
pub extern "C" fn ckb_inherited_fds(fds: *mut u64, length: *mut usize) -> c_int {
    let out_fds = get_cur_tx!().vm_info(&VMInfo::ctx_id()).inherited_fds();
    let len = out_fds.len().min(utils::to_usize(length));

    copy_fds(&out_fds[0..len], fds);
    unsafe { *({ length }) = len };
    0
}

#[no_mangle]
pub extern "C" fn ckb_close(fd: u64) -> c_int {
    let fd = fd.into();

    let r = get_cur_tx_mut!().close_pipe(fd);
    if r {
        0
    } else {
        6 // CKB_INVALID_FD
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

fn new_vm_id(inherited_fds: &[Fd]) -> VmID {
    let cur_id = VMInfo::ctx_id();
    let new_id = get_cur_tx_mut!().new_vm(Some(cur_id.clone()), inherited_fds);

    inherited_fds.iter().all(|fd| {
        get_cur_tx_mut!().move_pipe(fd, new_id.clone());
        true
    });

    new_id
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
                    return Err(6); // CKB_INVALID_FD
                }
            }
            Self::Write => {
                if fd.is_read() {
                    return Err(6); // CKB_INVALID_FD
                }
            }
        }

        let g = GlobalData::locked();
        let tx_ctx = g.get_tx(&TxContext::ctx_id());
        if !tx_ctx.has_fd(fd) {
            return Err(6); // CKB_INVALID_FD
        }
        if !tx_ctx.chech_other_fd(fd) {
            return Err(7); // OTHER_END_CLOSED
        }
        Ok(())
    }
}
