use crate::{
    global_data::{GlobalData, TxID, VmID},
    vm_info::{TxContext, VMInfo},
};
use std::{
    ffi::{c_int, c_void},
    path::PathBuf,
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

    pub fn ckb_std_main_async(self, argc: i32, argv: *const *const u8, pid: &VmID) {
        let args = to_vec_args(argc, argv as *const *const i8);
        let tx_ctx_id = TxContext::ctx_id();

        let pid2 = pid.clone();
        std::thread::spawn(move || {
            VMInfo::set_ctx_id(pid2.clone());
            TxContext::set_ctx_id(tx_ctx_id.clone());

            self.update_script_info(tx_ctx_id.clone(), pid2.clone());

            let rc = self.ckb_std_main(args);
            crate::get_vm!(&tx_ctx_id, &pid2).notify(None);
            rc
        });
    }

    pub fn update_script_info(&self, tx_ctx_id: TxID, vm_ctx_id: VmID) {
        type SetScriptInfo<'a> = libloading::Symbol<
            'a,
            unsafe extern "C" fn(ptr: *const c_void, tx_ctx_id: u64, vm_ctx_id: u64),
        >;

        unsafe {
            let func: SetScriptInfo = self
                .lib
                .get(b"__set_script_info")
                .expect("load function : __update_spawn_info");
            func(GlobalData::get_ptr(), tx_ctx_id.into(), vm_ctx_id.into())
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
