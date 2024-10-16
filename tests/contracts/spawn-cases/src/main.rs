#![cfg_attr(not(feature = "native-simulator"), no_std)]
#![cfg_attr(not(test), no_main)]

#[cfg(not(any(feature = "native-simulator", test)))]
ckb_std::entry!(program_entry);
#[cfg(not(any(feature = "native-simulator", test)))]
ckb_std::default_alloc!();

#[cfg(any(feature = "native-simulator", test))]
extern crate alloc;
use alloc::vec::Vec;
use ckb_std::ckb_types::bytes::Bytes;
use ckb_std::ckb_types::core::ScriptHashType;
use ckb_std::ckb_types::prelude::Unpack;
use ckb_std::syscalls::SysError;
use ckb_std::{debug, syscalls};
use ckb_std_wrapper::ckb_std;
use core::ffi::CStr;
use spawn_cmd::SpawnCasesCmd;

const CKB_STDIN: usize = 0;
const CKB_STDOUT: usize = 1;

fn error_to_code(err: SysError) -> u64 {
    match err {
        SysError::IndexOutOfBound => 1,
        SysError::ItemMissing => 2,
        SysError::LengthNotEnough(_usize) => 3,
        SysError::Encoding => 4,
        SysError::WaitFailure => 5,
        SysError::InvalidFd => 6,
        SysError::OtherEndClosed => 7,
        SysError::MaxVmsSpawned => 8,
        SysError::MaxFdsCreated => 9,
        SysError::Unknown(code) => code,
    }
}

fn create_std_fds() -> Result<([u64; 2], [u64; 3]), SysError> {
    let (r0, w0) = syscalls::pipe()?;
    let (r1, w1) = syscalls::pipe()?;
    Ok(([r0, w1], [r1, w0, 0]))
}

fn full_spawn(args: &[&str]) -> Result<(u64, [u64; 2]), SysError> {
    let (fds, inherited_fds) = create_std_fds()?;

    let info = ckb_std::high_level::load_cell_lock(0, ckb_std::ckb_constants::Source::GroupInput)?;
    let code_hash = info.code_hash();

    let args_buf: Vec<Vec<u8>> = args.iter().map(|f| [f.as_bytes(), &[0]].concat()).collect();

    let c_args: Vec<&CStr> = args_buf
        .iter()
        .map(|f| unsafe { CStr::from_bytes_with_nul_unchecked(f) })
        .collect();
    let pid = ckb_std::high_level::spawn_cell(
        &code_hash.raw_data(),
        ScriptHashType::Data2,
        &c_args,
        &inherited_fds,
    )?;

    Ok((pid, fds))
}

fn write_exact(fd: u64, buf: &[u8], actual_length: &mut usize) -> Result<(), SysError> {
    let mut w_buf = buf;
    *actual_length = 0;
    while !w_buf.is_empty() {
        let n = syscalls::write(fd, w_buf)?;
        *actual_length += n;
        w_buf = &w_buf[n..];
    }
    Ok(())
}

fn read_exact(fd: u64, buf: &mut [u8], actual_length: &mut usize) -> Result<(), SysError> {
    let mut r_buf = buf;
    *actual_length = 0;
    while !r_buf.is_empty() {
        let n = syscalls::read(fd, r_buf)?;
        *actual_length += n;
        r_buf = &mut r_buf[n..];
    }

    Ok(())
}

fn parent_simple_read_write() -> Result<Option<u64>, SysError> {
    let (pid, fds) = full_spawn(&[""])?;

    let block = [
        0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff,
    ];
    // write
    for _i in 0..7 {
        let mut actual_length = 0;
        debug!("-P- {} WBegin len: {}", _i, block.len());
        write_exact(fds[CKB_STDOUT], &block, &mut actual_length)?;
        if actual_length != block.len() {
            return Err(SysError::Unknown(-2i64 as u64));
        }
        debug!("-P- {} WEnd, actual_length: {}", _i, actual_length);
    }

    debug!("-P- --------");
    // read
    for _i in 0..7 {
        let mut actual_length = 0;
        let mut block = [0u8; 11];
        debug!("-P- {} RBegin len: {}", _i, block.len());
        read_exact(fds[CKB_STDIN], &mut block, &mut actual_length)?;

        if actual_length != block.len() {
            return Err(SysError::Unknown(-2i64 as u64));
        }
        if block.iter().any(|v| v != &0xff) {
            return Err(SysError::Unknown(-2i64 as u64));
        }
        debug!("-P- {} REnd actual_length: {}", _i, actual_length);
    }

    Ok(Some(pid))
}
fn child_simple_read_write() -> Result<(), SysError> {
    let mut inherited_fds = [0u64; 2];
    syscalls::inherited_fds(&mut inherited_fds);

    for _i in 0..11 {
        let mut block = [0u8; 7];
        let mut actual_length = 0;
        debug!("-C- {} RBegin len: {}", _i, block.len());
        read_exact(inherited_fds[CKB_STDIN], &mut block, &mut actual_length)?;
        if actual_length != block.len() {
            return Err(SysError::Unknown(-2i64 as u64));
        }
        if block.iter().any(|v| v != &0xff) {
            return Err(SysError::Unknown(-3i64 as u64));
        }
        debug!("-C- {} REnd, actual_length: {}", _i, actual_length);
    }

    debug!("-C- --------");
    let block = [
        0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff,
    ];
    for _i in 0..7 {
        debug!("-C- {} WBegin len: {}", _i, block.len());
        let mut actual_length = 0;
        write_exact(inherited_fds[CKB_STDOUT], &block, &mut actual_length)?;
        if actual_length != block.len() {
            return Err(SysError::Unknown(-2i64 as u64));
        }
        debug!("-C- {} WEnd actual_length: {}", _i, actual_length);
    }

    Ok(())
}

fn parent_write_dead_lock() -> Result<Option<u64>, SysError> {
    let (pid, fds) = full_spawn(&[""])?;

    let data = [0u8; 10];
    syscalls::write(fds[CKB_STDOUT], &data)?;

    Ok(Some(pid))
}
fn child_write_dead_lock() -> Result<(), SysError> {
    let mut inherited_fds = [0u64; 2];
    syscalls::inherited_fds(&mut inherited_fds);

    let data = [0u8; 10];
    syscalls::write(inherited_fds[CKB_STDOUT], &data)?;

    Ok(())
}

fn parent_invalid_fd() -> Result<Option<u64>, SysError> {
    let mut data = [0u8; 4];

    let invalid_fd = 0xff;
    let err = syscalls::read(invalid_fd, &mut data).unwrap_err();
    if err != SysError::InvalidFd {
        return Err(SysError::Unknown(-2i64 as u64));
    }

    let err = syscalls::write(invalid_fd, &data).unwrap_err();
    if err != SysError::InvalidFd {
        return Err(SysError::Unknown(-2i64 as u64));
    }

    let (r, w) = syscalls::pipe()?;
    let err = syscalls::read(w, &mut data).unwrap_err();
    if err != SysError::InvalidFd {
        return Err(SysError::Unknown(-2i64 as u64));
    }
    let err = syscalls::write(r, &data).unwrap_err();
    if err != SysError::InvalidFd {
        return Err(SysError::Unknown(-2i64 as u64));
    }

    // TODO

    Ok(None)
}

fn parent_entry(cmd: SpawnCasesCmd) -> i8 {
    debug!("-P- Begin cmd: {:?}", cmd);

    let ret = match cmd {
        SpawnCasesCmd::Unknow => panic!("pass"),
        SpawnCasesCmd::ReadWrite => parent_simple_read_write(),
        SpawnCasesCmd::WriteDeadLock => parent_write_dead_lock(),
        SpawnCasesCmd::InvalidFd => parent_invalid_fd(),
    };

    let code = match ret {
        Ok(pid) => {
            if let Some(pid) = pid {
                debug!("-P- Wait Child");
                let exit_code: i8 = syscalls::wait(pid).unwrap();
                exit_code
            } else {
                0
            }
        }
        Err(e) => error_to_code(e) as i8,
    };

    debug!("-P- End, code: {}", code);
    code
}
fn child_entry(cmd: SpawnCasesCmd) -> i8 {
    debug!("-C- Begin cmd: {:?}", cmd);
    let ret = match cmd {
        SpawnCasesCmd::Unknow => panic!("unsupport"),
        SpawnCasesCmd::ReadWrite => child_simple_read_write(),
        SpawnCasesCmd::WriteDeadLock => child_write_dead_lock(),
        SpawnCasesCmd::InvalidFd => panic!("unsupport"),
    };

    let code = match ret {
        Ok(_) => 0,
        Err(e) => error_to_code(e) as i8,
    };
    debug!("-C- End code: {:?}", code);
    code
}

pub fn program_entry() -> i8 {
    match ckb_std::high_level::load_script() {
        Ok(script) => {
            let script_args: Bytes = script.args().unpack();
            let script_args = script_args.to_vec();

            let cmd: SpawnCasesCmd = script_args[0].into();

            let argv = ckb_std::env::argv();

            if argv.is_empty() {
                parent_entry(cmd)
            } else {
                child_entry(cmd)
            }
        }
        Err(e) => {
            panic!("load script error: {:?}", e)
        }
    }
}
