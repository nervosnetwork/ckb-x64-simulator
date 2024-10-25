#![cfg_attr(not(feature = "native-simulator"), no_std)]
#![cfg_attr(not(test), no_main)]

#[cfg(any(feature = "native-simulator", test))]
extern crate alloc;

#[cfg(not(any(feature = "native-simulator", test)))]
use ckb_std::default_alloc;
#[cfg(not(any(feature = "native-simulator", test)))]
ckb_std::entry!(program_entry);
#[cfg(not(any(feature = "native-simulator", test)))]
default_alloc!();
use ckb_std_wrapper::ckb_std;

use ckb_std::ckb_types::bytes::Bytes;
use ckb_std::ckb_types::core::ScriptHashType;
use ckb_std::ckb_types::prelude::Unpack;
use ckb_std::debug;
use core::ffi::CStr;

pub fn program_entry() -> i8 {
    debug!("This is a sample contract exec-parent!");

    if let Ok(script) = ckb_std::high_level::load_script() {
        let args: Bytes = script.args().unpack();
        let args = args.to_vec();

        if args.len() < 33 {
            debug!("args len loss 33: {}", args.len());
            return 1;
        }
        let hash_type = match args[32] {
            0 => ScriptHashType::Data,
            1 => ScriptHashType::Type,
            2 => ScriptHashType::Data1,
            4 => ScriptHashType::Data2,
            _ => {
                debug!("unknow hash type : {}", args[32]);
                return 2;
            }
        };
        let arg1 = CStr::from_bytes_with_nul(b"Hello World\0").unwrap();
        let arg2 = CStr::from_bytes_with_nul("你好\0".as_bytes()).unwrap();

        let rc = ckb_std::high_level::exec_cell(&args[..32], hash_type, &[arg1, arg2]).unwrap_err();
        debug!("exec_cell faield: {:?}", rc);
        3
    } else {
        debug!("load script failed");
        4
    }
}
