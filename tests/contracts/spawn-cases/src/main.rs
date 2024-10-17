#![cfg_attr(not(feature = "native-simulator"), no_std)]
#![cfg_attr(not(test), no_main)]

#[cfg(not(any(feature = "native-simulator", test)))]
ckb_std::entry!(program_entry);
#[cfg(not(any(feature = "native-simulator", test)))]
ckb_std::default_alloc!();

#[cfg(any(feature = "native-simulator", test))]
extern crate alloc;
use alloc::vec::Vec;
use ckb_std::ckb_constants::Source;
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
fn new_spawn(args: &[&str], inherited_fds: &[u64]) -> Result<u64, SysError> {
    let args_buf: Vec<Vec<u8>> = args.iter().map(|f| [f.as_bytes(), &[0]].concat()).collect();
    let c_args: Vec<&CStr> = args_buf
        .iter()
        .map(|f| unsafe { CStr::from_bytes_with_nul_unchecked(f) })
        .collect();
    let pid = ckb_std::high_level::spawn_cell(
        &ckb_std::high_level::load_cell_lock(0, Source::GroupInput)?
            .code_hash()
            .raw_data(),
        ScriptHashType::Data2,
        &c_args,
        inherited_fds,
    )?;
    Ok(pid)
}
fn full_spawn(args: &[&str]) -> Result<(u64, [u64; 2]), SysError> {
    let (fds, inherited_fds) = create_std_fds()?;
    let pid = new_spawn(args, &inherited_fds)?;
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

    let inherited_fds = [r, 0];
    let args = [""];
    let args_buf: Vec<Vec<u8>> = args.iter().map(|f| [f.as_bytes(), &[0]].concat()).collect();
    let c_args: Vec<&CStr> = args_buf
        .iter()
        .map(|f| unsafe { CStr::from_bytes_with_nul_unchecked(f) })
        .collect();
    let pid = ckb_std::high_level::spawn_cell(
        &ckb_std::high_level::load_cell_lock(0, Source::GroupInput)?
            .code_hash()
            .raw_data(),
        ScriptHashType::Data2,
        &c_args,
        &inherited_fds,
    )?;

    let mut buf = [0u8; 4];
    let err = syscalls::read(r, &mut buf).unwrap_err();
    assert_eq!(err, SysError::InvalidFd);

    let (r1, w1) = syscalls::pipe()?;
    syscalls::close(r1)?;
    let buf = [0xfu8; 4];
    let err = syscalls::write(w1, &buf).unwrap_err();
    assert_eq!(err, SysError::OtherEndClosed);

    let (r1, w1) = syscalls::pipe()?;
    syscalls::close(w1)?;
    let mut buf = [0xfu8; 4];
    let err = syscalls::read(r1, &mut buf).unwrap_err();
    assert_eq!(err, SysError::OtherEndClosed);

    syscalls::wait(pid)?;

    Ok(None)
}

fn parent_wait_dead_lock() -> Result<Option<u64>, SysError> {
    let (pid, _) = full_spawn(&[""])?;
    Ok(Some(pid))
}
fn child_wait_dead_lock() -> Result<(), SysError> {
    let pid = 0;
    syscalls::wait(pid)?;
    Ok(())
}

fn parent_read_write_with_close() -> Result<Option<u64>, SysError> {
    let (pid, fds) = full_spawn(&[""])?;
    let block = [0xFFu8; 100];
    let mut actual_length = 0;
    write_exact(fds[CKB_STDOUT], &block, &mut actual_length)?;

    if actual_length != block.len() {
        return Err(SysError::Unknown(-2i64 as u64));
    }

    Ok(Some(pid))
}
fn child_read_write_with_close() -> Result<(), SysError> {
    let mut inherited_fds = [0u64; 2];
    syscalls::inherited_fds(&mut inherited_fds);

    let mut block = [0u8; 100];
    let mut actual_length = 0;
    read_exact(inherited_fds[CKB_STDIN], &mut block, &mut actual_length)?;
    if actual_length != block.len() {
        return Err(SysError::Unknown(-2i64 as u64));
    }

    if block.iter().any(|v| v != &0xFF) {
        return Err(SysError::Unknown(-3i64 as u64));
    }

    syscalls::close(inherited_fds[CKB_STDIN])?;

    Ok(())
}

fn parent_wait_multiple() -> Result<Option<u64>, SysError> {
    let (pid, _fds) = full_spawn(&[""])?;

    let exit_code = syscalls::wait(pid)?;
    assert_eq!(exit_code, 0);

    let err = syscalls::wait(pid).unwrap_err();
    assert_eq!(err, SysError::WaitFailure);

    let (pid, _fds) = full_spawn(&[""])?;

    Ok(Some(pid))
}

fn parent_inherited_fds() -> Result<Option<u64>, SysError> {
    let mut inherited_fds = [0u64; 11];
    for i in 0..5 {
        let (r, w) = syscalls::pipe()?;
        inherited_fds[i * 2] = r;
        inherited_fds[i * 2 + 1] = w;
    }

    let pid = new_spawn(&[""], &inherited_fds)?;
    Ok(Some(pid))
}
fn child_inherited_fds() -> Result<(), SysError> {
    let mut inherited_fds = [0u64; 10];
    syscalls::inherited_fds(&mut inherited_fds);

    for i in 0u64..10u64 {
        if inherited_fds[i as usize] != i + 2 {
            return Err(SysError::Unknown(-2i64 as u64));
        }
    }

    Ok(())
}

fn parent_inherited_fds_without_owner() -> Result<Option<u64>, SysError> {
    let fds = [0xFF, 0xFF, 0];
    let err = new_spawn(&[""], &fds).unwrap_err();
    assert_eq!(err, SysError::InvalidFd);

    let (r, w) = syscalls::pipe()?;

    let pid = new_spawn(&[""], &[r, w, 0])?;
    let err = new_spawn(&[""], &[r, w, 0]).unwrap_err();
    assert_eq!(err, SysError::InvalidFd);

    Ok(Some(pid))
}

fn parent_read_then_close() -> Result<Option<u64>, SysError> {
    let (pid, fds) = full_spawn(&[""])?;
    debug!("-P- Spawn end, fds: {:?}", fds);
    syscalls::close(fds[CKB_STDOUT])?;
    debug!("-P- Close fd: {}", fds[CKB_STDOUT]);
    Ok(Some(pid))
}
fn child_read_then_close() -> Result<(), SysError> {
    let mut fds = [0u64; 2];
    syscalls::inherited_fds(&mut fds);
    debug!("-C- inherited fds {:?}", fds);

    let mut data = [0u8; 8];
    debug!("-C- Read begin");
    let data_len = syscalls::read(fds[CKB_STDIN], &mut data)?;
    debug!("-C- data len : {}", data_len);

    let err = syscalls::read(fds[CKB_STDIN], &mut data).unwrap_err();
    if err != SysError::OtherEndClosed {
        return Err(SysError::Unknown(-2i64 as u64));
    }

    Ok(())
}

fn parent_max_vms_count() -> Result<Option<u64>, SysError> {
    let pid = new_spawn(&[""], &[0])?;
    debug!("-P- pid: {}", pid);
    Ok(Some(pid))
}
fn child_max_vms_count() -> Result<(), SysError> {
    match new_spawn(&[""], &[0]) {
        Ok(_pid) => {
            debug!("-C- pid: {}", _pid);
            Ok(())
        }
        Err(e) => {
            if e == SysError::MaxVmsSpawned {
                Ok(())
            } else {
                Err(e)
            }
        }
    }
}

fn parent_max_fds_limit() -> Result<Option<u64>, SysError> {
    for _ in 0..16 {
        let _ = syscalls::pipe()?;
    }

    new_spawn(&[""], &[0])?;
    Ok(None)
}
fn child_max_fds_limit() -> Result<(), SysError> {
    for _ in 0..16 {
        let _ = syscalls::pipe()?;
    }

    let err = syscalls::pipe().unwrap_err();
    assert_eq!(err, SysError::MaxFdsCreated);

    Ok(())
}

fn parent_close_invalid_fd() -> Result<Option<u64>, SysError> {
    let fds = syscalls::pipe()?;

    let err = syscalls::close(fds.0 + 32).unwrap_err();
    assert_eq!(err, SysError::InvalidFd);

    syscalls::close(fds.0)?;
    syscalls::close(fds.1)?;

    let err = syscalls::close(fds.0).unwrap_err();
    assert_eq!(err, SysError::InvalidFd);
    let err = syscalls::close(fds.1).unwrap_err();
    assert_eq!(err, SysError::InvalidFd);

    Ok(None)
}

fn parent_write_closed_fd() -> Result<Option<u64>, SysError> {
    let (pid, fds) = full_spawn(&[""])?;

    let mut block = [0u8; 7];
    let mut actual_length = 0;
    debug!("-P- Read begin");
    read_exact(fds[CKB_STDIN], &mut block, &mut actual_length)?;
    debug!("-P- Read end");

    assert_eq!(actual_length, block.len());

    debug!("-P- Close1 begin");
    syscalls::close(fds[CKB_STDIN])?;
    debug!("-P- Close2 begin");
    syscalls::close(fds[CKB_STDOUT])?;
    debug!("-P- Close end");

    Ok(Some(pid))
}
fn child_write_closed_fd() -> Result<(), SysError> {
    let mut fds = [0, 2];
    syscalls::inherited_fds(&mut fds);

    debug!("-C- Write1 begin");
    let block = [0u8; 7];
    let mut actual_length = 0;
    write_exact(fds[CKB_STDOUT], &block, &mut actual_length)?;
    debug!("-C- Write2 begin");
    let err = write_exact(fds[CKB_STDOUT], &block, &mut actual_length).unwrap_err();
    assert_eq!(err, SysError::OtherEndClosed);
    debug!("-C- Write end");

    debug!("-C- Close1 begin");
    syscalls::close(fds[CKB_STDIN])?;
    debug!("-C- Close2 begin");
    syscalls::close(fds[CKB_STDOUT])?;
    debug!("-C- Close end");

    Ok(())
}

fn parent_spawn_pid() -> Result<Option<u64>, SysError> {
    let cur_pid = syscalls::process_id();
    assert_eq!(cur_pid, 0);

    let (pid_1, fds_1) = full_spawn(&[""])?;
    assert_eq!(pid_1, 1);

    let (pid_2, fds_2) = full_spawn(&[""])?;
    assert_eq!(pid_2, 2);

    let mut buf = [0u8; 8];
    let mut actual_length = 0;
    read_exact(fds_1[CKB_STDIN], &mut buf, &mut actual_length)?;
    assert_eq!(pid_1, u64::from_le_bytes(buf));

    let mut buf = [0u8; 8];
    let mut actual_length = 0;
    read_exact(fds_2[CKB_STDIN], &mut buf, &mut actual_length)?;
    assert_eq!(pid_2, u64::from_le_bytes(buf));

    Ok(None)
}
fn child_spawn_pid() -> Result<(), SysError> {
    let pid = syscalls::process_id();

    let mut fds = [0; 2];
    syscalls::inherited_fds(&mut fds);

    let mut actual_length = 0;
    write_exact(fds[CKB_STDOUT], &pid.to_le_bytes(), &mut actual_length)?;

    Ok(())
}

fn parent_entry(cmd: SpawnCasesCmd) -> i8 {
    debug!("-P- Begin cmd: {:?}, pid: {}", cmd, syscalls::process_id());

    let ret = match cmd {
        SpawnCasesCmd::Unknow => panic!("pass"),
        SpawnCasesCmd::ReadWrite => parent_simple_read_write(),
        SpawnCasesCmd::WriteDeadLock => parent_write_dead_lock(),
        SpawnCasesCmd::InvalidFd => parent_invalid_fd(),
        SpawnCasesCmd::WaitDeadLock => parent_wait_dead_lock(),
        SpawnCasesCmd::ReadWriteWithClose => parent_read_write_with_close(),
        SpawnCasesCmd::WaitMultiple => parent_wait_multiple(),
        SpawnCasesCmd::InheritedFds => parent_inherited_fds(),
        SpawnCasesCmd::InheritedFdsWithoutOwner => parent_inherited_fds_without_owner(),
        SpawnCasesCmd::ReadThenClose => parent_read_then_close(),
        SpawnCasesCmd::MaxVmsCount => parent_max_vms_count(),
        SpawnCasesCmd::MaxFdsLimit => parent_max_fds_limit(),
        SpawnCasesCmd::CloseInvalidFd => parent_close_invalid_fd(),
        SpawnCasesCmd::WriteClosedFd => parent_write_closed_fd(),
        SpawnCasesCmd::CheckPID => parent_spawn_pid(),
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
    debug!("-C- Begin cmd: {:?}, pid: {}", cmd, syscalls::process_id());
    let ret = match cmd {
        SpawnCasesCmd::Unknow => panic!("unsupport"),
        SpawnCasesCmd::ReadWrite => child_simple_read_write(),
        SpawnCasesCmd::WriteDeadLock => child_write_dead_lock(),
        SpawnCasesCmd::InvalidFd => Ok(()),
        SpawnCasesCmd::WaitDeadLock => child_wait_dead_lock(),
        SpawnCasesCmd::ReadWriteWithClose => child_read_write_with_close(),
        SpawnCasesCmd::WaitMultiple => Ok(()),
        SpawnCasesCmd::InheritedFds => child_inherited_fds(),
        SpawnCasesCmd::InheritedFdsWithoutOwner => Ok(()),
        SpawnCasesCmd::ReadThenClose => child_read_then_close(),
        SpawnCasesCmd::MaxVmsCount => child_max_vms_count(),
        SpawnCasesCmd::MaxFdsLimit => child_max_fds_limit(),
        SpawnCasesCmd::CloseInvalidFd => Ok(()),
        SpawnCasesCmd::WriteClosedFd => child_write_closed_fd(),
        SpawnCasesCmd::CheckPID => child_spawn_pid(),
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
