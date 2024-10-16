use crate::global_data::GlobalData;
use std::{
    ffi::{c_int, c_void},
    path::PathBuf,
    sync::{Arc, Condvar, Mutex},
};

pub fn get_simulator_path(
    code_hash: &[u8],
    hash_type: u8,
    offset: u32,
    length: u32,
) -> Option<String> {
    let mut filename = None;
    for ht in [hash_type, 0xFF] {
        let mut buffer = vec![];
        buffer.extend_from_slice(code_hash);
        buffer.push(ht);
        buffer.extend_from_slice(&offset.to_be_bytes()[..]);
        buffer.extend_from_slice(&length.to_be_bytes()[..]);
        let key = format!("0x{}", faster_hex::hex_string(&buffer));
        filename = crate::SETUP.native_binaries.get(&key);
        if filename.is_some() {
            break;
        }
    }
    filename.cloned()
}

pub struct CkbNativeSimulator {
    lib: libloading::Library,
}
impl CkbNativeSimulator {
    pub fn new_by_hash(code_hash: *const u8, hash_type: u8, offset: u32, length: u32) -> Self {
        let sim_path = get_simulator_path(
            unsafe { std::slice::from_raw_parts(code_hash, 32) },
            hash_type,
            offset,
            length,
        );
        let sim_path = sim_path.expect("cannot locate native binary for ckb_spawn syscall!");
        Self::new(&sim_path.into())
    }
    fn new(path: &PathBuf) -> Self {
        unsafe {
            let lib = libloading::Library::new(path).expect("Load library");
            Self { lib }
        }
    }

    pub fn ckb_std_main(self, args: Vec<String>) -> i8 {
        type CkbMainFunc<'a> =
            libloading::Symbol<'a, unsafe extern "C" fn(argc: i32, argv: *const *const i8) -> i8>;

        let argc = args.len() as u64;
        let mut argv: Vec<*const i8> = Vec::with_capacity(argc as usize + 1);
        for s in args {
            let c_string = std::ffi::CString::new(s.clone()).expect("CString::new failed");
            argv.push(c_string.into_raw());
        }
        argv.push(std::ptr::null_mut());

        unsafe {
            let func: CkbMainFunc = self
                .lib
                .get(b"__ckb_std_main")
                .expect("load function : __ckb_std_main");
            func(argc as i32, argv.as_ptr())
        }
    }

    pub fn update_script_info(&self, tx_ctx_id: SimID, pid: ProcID) {
        type SetScriptInfo<'a> = libloading::Symbol<
            'a,
            unsafe extern "C" fn(ptr: *const c_void, tx_ctx_id: u64, pid: u64),
        >;

        unsafe {
            let func: SetScriptInfo = self
                .lib
                .get(b"__set_script_info")
                .expect("load function : __update_spawn_info");
            func(GlobalData::get_ptr(), tx_ctx_id.into(), pid.into())
        }
    }
}

pub fn to_vec_args(argc: c_int, argv: *const *const i8) -> Vec<String> {
    let mut args = Vec::with_capacity(argc as usize);
    for i in 0..argc {
        let c_str = unsafe { std::ffi::CStr::from_ptr(*argv.add(i as usize)) };
        let str_slice = c_str
            .to_str()
            .expect("Failed to convert C string to Rust string");
        args.push(str_slice.to_owned());
    }
    args
}

pub fn to_array(ptr: *const u8, len: usize) -> &'static [u8] {
    unsafe { std::slice::from_raw_parts(ptr, len) }
}

pub fn to_c_str(ptr: *const std::ffi::c_char) -> &'static core::ffi::CStr {
    unsafe { core::ffi::CStr::from_ptr(ptr) }
}

pub fn to_usize(ptr: *mut usize) -> usize {
    unsafe { *ptr }
}

#[derive(Default, Debug)]
pub struct Event {
    data: Arc<(Mutex<bool>, Condvar)>,
}
impl Clone for Event {
    fn clone(&self) -> Self {
        Self {
            data: Arc::clone(&self.data),
        }
    }
}
impl Event {
    pub fn notify(&self) {
        let (lock, cvar) = &*self.data;
        let mut started = lock.lock().unwrap();
        *started = true;
        cvar.notify_one();
    }

    pub fn wait(&self) {
        let (lock, cvar) = &*self.data;
        let mut started = lock.lock().unwrap();

        loop {
            if *started {
                *started = false;
                break;
            }
            started = cvar.wait(started).unwrap();
        }
    }
}

#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Fd(pub u64);
impl From<u64> for Fd {
    fn from(value: u64) -> Self {
        Self(value)
    }
}
impl From<Fd> for u64 {
    fn from(value: Fd) -> Self {
        value.0
    }
}
impl std::fmt::Debug for Fd {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "FD:{}", self.0)
    }
}
impl Fd {
    pub fn create(slot: u64) -> (Fd, Fd, u64) {
        (Fd(slot), Fd(slot + 1), slot + 2)
    }
    pub fn other_fd(&self) -> Fd {
        Fd(self.0 ^ 0x1)
    }
    pub fn is_read(&self) -> bool {
        self.0 % 2 == 0
    }
}

#[derive(Default, PartialEq, Eq, Clone, Hash)]
pub struct SimID(u64);
impl From<u64> for SimID {
    fn from(value: u64) -> Self {
        Self(value)
    }
}
impl From<SimID> for u64 {
    fn from(value: SimID) -> Self {
        value.0
    }
}
impl std::fmt::Debug for SimID {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "SID:{}", self.0)
    }
}
impl SimID {
    pub fn next(&mut self) -> Self {
        self.0 += 1;
        self.clone()
    }
}

#[derive(Default, PartialEq, Eq, PartialOrd, Ord, Clone, Hash)]
pub struct ProcID(u64);
impl From<u64> for ProcID {
    fn from(value: u64) -> Self {
        Self(value)
    }
}
impl From<ProcID> for u64 {
    fn from(value: ProcID) -> Self {
        value.0
    }
}
impl std::fmt::Debug for ProcID {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "PID:{}", self.0)
    }
}
impl ProcID {
    pub fn next(&mut self) -> Self {
        let id = self.clone();
        self.0 += 1;
        id
    }
}
