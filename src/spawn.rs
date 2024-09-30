use crate::{
    get_tx, get_tx_mut,
    global_data::{GlobalData, TxID, VmID},
    utils,
    vm_info::{Fd, TxContext, VMInfo},
};
use std::os::raw::{c_int, c_void};

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
    let ckb_sim = utils::CkbNativeSimulator::new_by_hash(code_hash, hash_type, offset, length);
    let (rc, id) = Spawn::default().spawn_cell(ckb_sim, argc, argv, inherited_fds);
    if let Some(id) = id {
        unsafe { *({ pid }) = id.into() };
    }

    rc
}

#[no_mangle]
pub extern "C" fn ckb_wait(_pid: u64, _code: *mut i8) -> c_int {
    panic!("unsupport");
}

#[no_mangle]
pub extern "C" fn ckb_process_id() -> u64 {
    Spawn::default().pid()
}

#[no_mangle]
pub extern "C" fn ckb_pipe(fds: *mut u64) -> c_int {
    Spawn::default().pipe(fds)
}

#[no_mangle]
pub extern "C" fn ckb_read(fd: u64, buf: *mut c_void, length: *mut usize) -> c_int {
    Spawn::default().read(fd.into(), buf, length)
}

#[no_mangle]
pub extern "C" fn ckb_write(fd: u64, buf: *const c_void, length: *mut usize) -> c_int {
    Spawn::default().write(fd.into(), buf, length)
}

#[no_mangle]
pub extern "C" fn ckb_inherited_fds(fds: *mut u64, length: *mut usize) -> c_int {
    Spawn::default().inherited_fds(fds, length)
}

#[no_mangle]
pub extern "C" fn ckb_close(fd: u64) -> c_int {
    Spawn::default().close(fd)
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

struct Spawn {
    tx_id: TxID,
    vm_id: VmID,
}

const MAX_FDS: usize = 64;

impl Default for Spawn {
    fn default() -> Self {
        Self {
            tx_id: TxContext::ctx_id(),
            vm_id: VMInfo::ctx_id(),
        }
    }
}
impl Spawn {
    fn pid(&self) -> u64 {
        self.vm_id.clone().into()
    }
    fn pipe(&self, fds: *mut u64) -> i32 {
        if get_tx!(&self.tx_id).len_pipe() >= MAX_FDS {
            return 9; // MAX_FDS_CREATED
        }

        let out = get_tx_mut!(&TxContext::ctx_id()).new_pipe();
        Self::copy_fd(&[out.0, out.1], fds);
        0
    }
    fn close(&self, fd: u64) -> i32 {
        let fd = fd.into();

        let r = get_tx_mut!(self.tx_id).close_pipe(fd);
        if r {
            0
        } else {
            6 // CKB_INVALID_FD
        }
    }

    fn spawn_cell(
        &self,
        ckb_sim: utils::CkbNativeSimulator,
        argc: i32,
        argv: *const *const u8,
        inherited_fds: *const u64,
    ) -> (i32, Option<VmID>) {
        let new_id = self.new_vm_id(inherited_fds);
        ckb_sim.ckb_std_main_async(argc, argv, &new_id);

        let event = crate::get_cur_vm!().wait_by_pid(&new_id);
        event.wait();

        (0, Some(new_id))
    }
    fn inherited_fds(&self, fds: *mut u64, length: *mut usize) -> i32 {
        let out_fds = get_tx!(&TxContext::ctx_id())
            .vm_info(&VMInfo::ctx_id())
            .inherited_fds();
        let len = out_fds.len().min(unsafe { *length });

        Self::copy_fd(&out_fds[0..len], fds);
        0
    }

    fn read(&self, fd: Fd, buf: *mut c_void, length: *mut usize) -> c_int {
        // Check
        if let Err(e) = Self::check_fd(true, &fd) {
            return e;
        }
        crate::get_cur_vm!().notify(Some(&fd));
        let event = crate::get_cur_vm!().wait(Some(&fd));
        event.wait();

        let buf_len = unsafe { *length };

        let data = get_tx_mut!(&TxContext::ctx_id()).read_data(&fd, buf_len);
        if !data.is_empty() {
            unsafe { std::ptr::copy_nonoverlapping(data.as_ptr(), buf as *mut u8, data.len()) };
        }
        unsafe { *length = data.len() };

        0
    }
    fn write(&self, fd: Fd, buf: *const c_void, length: *mut usize) -> c_int {
        if let Err(e) = Self::check_fd(false, &fd) {
            return e;
        }
        let has_data = get_tx_mut!(&TxContext::ctx_id()).has_data(&fd);

        if has_data {
            crate::get_cur_vm!().notify(Some(&fd));
            let event = crate::get_cur_vm!().wait(Some(&fd));
            event.wait();
        }

        // TODO 需要写个case去判断这里的逻辑
        if buf.is_null() || unsafe { *length == 0 } {
            unsafe {
                *length = 0;
            }
            return 0;
        }
        let buf = unsafe {
            let length = *length;
            std::slice::from_raw_parts(buf as *const u8, length)
        }
        .to_vec();
        get_tx_mut!(&TxContext::ctx_id()).write_data(&fd, &buf);

        if !has_data {
            crate::get_cur_vm!().notify(Some(&fd));
            let event = crate::get_cur_vm!().wait(Some(&fd));
            event.wait();
        }

        0
    }

    fn copy_fd(in_fd: &[Fd], out_fd: *mut u64) {
        let mut out_fd = out_fd;
        for fd in in_fd {
            unsafe {
                *out_fd = fd.clone().into();
                out_fd = out_fd.add(1);
            }
        }
    }
    fn new_vm_id(&self, inherited_fds: *const u64) -> VmID {
        let inherited_fds: Vec<Fd> = unsafe {
            let mut fds = Vec::new();
            let mut fds_ptr = inherited_fds;
            while *fds_ptr != 0 {
                fds.push((*fds_ptr).into());
                fds_ptr = fds_ptr.add(1);
            }
            fds
        };
        let vm_id = get_tx_mut!(self.tx_id).new_vm(Some(self.vm_id.clone()), &inherited_fds);
        inherited_fds.iter().all(|fd| {
            get_tx_mut!(self.tx_id).move_pipe(fd, vm_id.clone());
            true
        });

        vm_id
    }
    fn check_fd(is_read: bool, fd: &Fd) -> Result<(), c_int> {
        if fd.is_read() != is_read {
            return Err(6); // CKB_INVALID_FD
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
