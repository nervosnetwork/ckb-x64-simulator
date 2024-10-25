#![cfg_attr(not(feature = "native-simulator"), no_std)]
#![cfg_attr(not(test), no_main)]

#[cfg(not(any(feature = "native-simulator", test)))]
ckb_std::entry!(program_entry);
#[cfg(not(any(feature = "native-simulator", test)))]
ckb_std::default_alloc!();

#[cfg(any(feature = "native-simulator", test))]
extern crate alloc;
use ckb_std_wrapper::ckb_std;

use alloc::{
    string::{String, ToString},
    vec,
    vec::Vec,
};
use ckb_std::{
    ckb_types::{bytes::Bytes, packed::Byte32, prelude::Unpack},
    debug,
    error::SysError,
    syscalls,
};
use core::ffi::CStr;
use spawn_cmd::SpawnCmd;

pub fn program_entry() -> i8 {
    debug!("-A- SpawnParent(pid:{}) Begin --", syscalls::process_id());

    let rc = SpawnArgs::default().cmd_routing();
    debug!("-A- Spawn-Parent(pid:{}) End --", syscalls::process_id());
    rc
}

#[derive(Clone)]
struct SpawnArgs {
    cmd: SpawnCmd,
    code_hash: Byte32,
    _args: Vec<u8>,
}
impl Default for SpawnArgs {
    fn default() -> Self {
        let args = {
            let script = ckb_std::high_level::load_script().expect("Load script");
            let args: Bytes = script.args().unpack();
            args.to_vec()
        };

        let cmd = args[0].into();
        let code_hash = Byte32::new(args[1..33].to_vec().try_into().unwrap());
        let args = args[33..].to_vec();

        Self {
            cmd,
            code_hash,
            _args: args,
        }
    }
}
impl SpawnArgs {
    fn cmd_routing(self) -> i8 {
        debug!("-A- cmd: {:?}", self.cmd);
        match self.cmd {
            SpawnCmd::Base => spawn_base(self),
            SpawnCmd::SpawnRetNot0 => spawn_ret_not0(self),
            SpawnCmd::WaitRetNot0 => wait_ret_not0(self),
            SpawnCmd::WaitInvalidPid => wait_invalid_pid(self),
            SpawnCmd::EmptyPipe => spawn_empty_pipe(self),
            SpawnCmd::SpawnInvalidFd => spawn_invalid_fd(self),
            SpawnCmd::SpawnMaxVms => spawn_max_vms(self),
            SpawnCmd::PipeMaxFds => pipe_max_fds(self),
            SpawnCmd::BaseIO1 => spawn_base_io1(self),
            SpawnCmd::BaseIO2 => spawn_base_io2(self),
            SpawnCmd::BaseIO3 => spawn_base_io3(self),
            SpawnCmd::BaseIO4 => spawn_base_io4(self),
            SpawnCmd::IOReadMore => io_read_more(self),
            SpawnCmd::IOWriteMore => io_write_more(self),
        }
    }

    fn new_spawn(self, args: &[String], fds: &[u64]) -> Result<u64, SysError> {
        let cmd: u8 = self.cmd.into();
        let args = [&[cmd.to_string()], args].concat();
        let args: Vec<Vec<u8>> = args
            .iter()
            .map(|s| alloc::vec![s.as_bytes(), &[0u8]].concat())
            .collect();
        let argv: Vec<&CStr> = args
            .iter()
            .map(|s| CStr::from_bytes_until_nul(s).unwrap())
            .collect();

        ckb_std::high_level::spawn_cell(
            &self.code_hash.raw_data(),
            ckb_std::ckb_types::core::ScriptHashType::Data2,
            &argv,
            fds,
        )
    }
}

fn new_pipe() -> ([u64; 2], [u64; 3]) {
    let mut std_fds: [u64; 2] = [0, 0];
    let mut son_fds: [u64; 3] = [0, 0, 0];
    let (r0, w0) = syscalls::pipe().unwrap();
    std_fds[0] = r0;
    son_fds[1] = w0;
    let (r1, w1) = syscalls::pipe().unwrap();
    std_fds[1] = w1;
    son_fds[0] = r1;
    (std_fds, son_fds)
}

fn spawn_base(args: SpawnArgs) -> i8 {
    let (std_fds, son_fds) = new_pipe();
    let pid = args.new_spawn(&[], &son_fds).expect("run spawn base");
    assert_eq!(pid, 1);

    assert!(syscalls::close(std_fds[0]).is_ok());
    assert!(syscalls::close(std_fds[1]).is_ok());

    assert_eq!(syscalls::close(son_fds[0]), Err(SysError::InvalidFd));
    assert_eq!(syscalls::close(son_fds[1]), Err(SysError::InvalidFd));

    assert_eq!(syscalls::process_id(), 0);

    let rr = syscalls::close(pid);
    assert_eq!(rr.unwrap_err(), SysError::InvalidFd);

    let code = syscalls::wait(pid).unwrap();
    assert_eq!(code, 0);
    0
}

fn spawn_ret_not0(args: SpawnArgs) -> i8 {
    let (_std_fds, son_fds) = new_pipe();
    let pid = args.new_spawn(&[], &son_fds).expect("run spawn base");
    assert_eq!(pid, 1);
    let code = syscalls::wait(pid).unwrap();
    assert_eq!(code, 3);

    0
}

fn wait_ret_not0(args: SpawnArgs) -> i8 {
    let (r0, _w0) = syscalls::pipe().unwrap();
    let pid = args.new_spawn(&[], &[r0, 0]).expect("run spawn base r");
    assert_eq!(pid, 1);

    let rc_code = syscalls::wait(pid).unwrap();

    assert_eq!(rc_code, 2);
    debug!("-A- rc code: {}", rc_code);
    0
}

fn wait_invalid_pid(args: SpawnArgs) -> i8 {
    let mut args = args;
    args.cmd = SpawnCmd::WaitRetNot0;
    let (r0, w0) = syscalls::pipe().unwrap();
    let pid = args.new_spawn(&[], &[r0, 0]).unwrap();
    assert_eq!(pid, 1);

    let err = syscalls::wait(pid + 2).unwrap_err();
    assert_eq!(err, SysError::WaitFailure);

    debug!("-A- Unlock B");
    syscalls::write(w0, &[2u8; 8]).unwrap();

    debug!("-A- Wait B");
    let rc_code = syscalls::wait(pid).unwrap();
    assert_eq!(rc_code, 2);

    0
}

fn spawn_empty_pipe(_args: SpawnArgs) -> i8 {
    let (std_fds, son_fds) = new_pipe();

    assert_eq!(std_fds[0], 2);
    assert_eq!(son_fds[1], 3);
    assert_eq!(son_fds[0], 4);
    assert_eq!(std_fds[1], 5);

    assert!(syscalls::close(std_fds[0]).is_ok());
    assert_eq!(syscalls::close(std_fds[0]), Err(SysError::InvalidFd));
    assert!(syscalls::close(std_fds[1]).is_ok());
    assert!(syscalls::close(son_fds[0]).is_ok());
    assert!(syscalls::close(son_fds[1]).is_ok());
    0
}

fn spawn_invalid_fd(args: SpawnArgs) -> i8 {
    let (_std_fds, son_fds) = new_pipe();
    let mut son_fds2 = son_fds;
    son_fds2[0] += 20;
    let err = args.new_spawn(&[], &son_fds2).unwrap_err();
    assert_eq!(err, ckb_std::error::SysError::InvalidFd);
    0
}

fn spawn_max_vms(args: SpawnArgs) -> i8 {
    for _ in 0..16 {
        let (r0, _w0) = syscalls::pipe().unwrap();
        let _pid = args.clone().new_spawn(&[], &[r0, 0]).unwrap();
    }

    let (r0, _w0) = syscalls::pipe().unwrap();
    let err = args.clone().new_spawn(&[], &[r0, 0]).unwrap_err();
    assert_eq!(err, SysError::MaxVmsSpawned);

    0
}

fn pipe_max_fds(args: SpawnArgs) -> i8 {
    // lock B
    let mut args = args;
    args.cmd = SpawnCmd::WaitRetNot0;
    let (r0, w0) = syscalls::pipe().unwrap();
    let pid = args.clone().new_spawn(&[], &[r0, 0]).unwrap();

    let mut fds = Vec::with_capacity(32);
    for _ in 0..31 {
        fds.push(syscalls::pipe().unwrap());
    }

    let err = syscalls::pipe().unwrap_err();
    assert_eq!(err, SysError::MaxFdsCreated);

    let (fd1, fd2) = fds.pop().unwrap();
    syscalls::close(fd1).unwrap();
    syscalls::close(fd2).unwrap();

    fds.push(syscalls::pipe().unwrap());

    while let Some((fd1, fd2)) = fds.pop() {
        syscalls::close(fd1).unwrap();
        syscalls::close(fd2).unwrap();
    }

    syscalls::write(w0, &[2u8; 8]).unwrap();
    let code = syscalls::wait(pid).unwrap();
    assert_eq!(code, 2);
    0
}

fn spawn_base_io1(args: SpawnArgs) -> i8 {
    let (std_fds, son_fds) = new_pipe();

    let argv = ["hello".to_string(), "world".to_string()];
    debug!("-A- Spawn --");
    let pid = args.new_spawn(&argv, &son_fds).expect("run spawn base io");
    debug!("-A- Spawn End, pid: {} --", pid);
    assert_eq!(pid, 1);

    let mut buf = [0; 10];
    let err = syscalls::read(std_fds[0] + 20, &mut buf).unwrap_err();
    assert_eq!(err, SysError::InvalidFd);

    debug!("-A- Read --");
    let mut buf = [0; 10];
    let len = syscalls::read(std_fds[0], &mut buf).expect("read 1");
    debug!("-A- Read {} End --", len);

    assert_eq!(len, 10);
    let buf = [buf.as_slice(), &[0]].concat();
    assert_eq!(
        CStr::from_bytes_until_nul(&buf).unwrap().to_str().unwrap(),
        "helloworld"
    );
    0
}

fn spawn_base_io2(args: SpawnArgs) -> i8 {
    let (std_fds, son_fds) = new_pipe();

    let argv = ["hello".to_string(), "world".to_string()];
    let pid = args.new_spawn(&argv, &son_fds).expect("run spawn base io");
    assert_eq!(pid, 1);

    debug!("-A- Write --");
    let write_buf = alloc::vec![argv[0].as_bytes(), argv[1].as_bytes()].concat();
    let len = syscalls::write(std_fds[1], &write_buf).expect("write");
    debug!("-A- Write End --");
    assert_eq!(len, write_buf.len());

    0
}

fn spawn_base_io3(args: SpawnArgs) -> i8 {
    let (std_fds, son_fds) = new_pipe();

    let argv = ["hello".to_string(), "world".to_string()];
    let pid = args.new_spawn(&argv, &son_fds).expect("run spawn base io");
    assert_eq!(pid, 1);

    let write_buf = alloc::vec![argv[0].as_bytes(), argv[1].as_bytes()].concat();
    let len = syscalls::write(std_fds[1], &write_buf).expect("write");
    assert_eq!(len, write_buf.len());

    0
}

fn spawn_base_io4(args: SpawnArgs) -> i8 {
    let (std_fds, son_fds) = new_pipe();

    let argv = ["hello".to_string(), "world".to_string()];
    let _pid = args.new_spawn(&argv, &son_fds).expect("run spawn base io");

    let mut buf1 = [0u8; 5];
    syscalls::read(std_fds[0], &mut buf1).unwrap();
    debug!("-A- buf1: {:02x?}", buf1);
    assert_eq!(
        CStr::from_bytes_until_nul(&[buf1.to_vec(), vec![0]].concat())
            .unwrap()
            .to_str()
            .unwrap(),
        "hello"
    );

    let mut buf2 = [0u8; 5];
    syscalls::read(std_fds[0], &mut buf2).unwrap();
    debug!("-A- buf2: {:02x?}", buf2);
    assert_eq!(
        CStr::from_bytes_until_nul(&[buf2.to_vec(), vec![0]].concat())
            .unwrap()
            .to_str()
            .unwrap(),
        "world"
    );

    0
}

fn io_read_more(args: SpawnArgs) -> i8 {
    let (fd_r, fd_w) = syscalls::pipe().unwrap();

    let pid = args.new_spawn(&[], &[fd_w, 0]).expect("run spawn base io");

    let mut buffer = [0u8; 128];

    debug!("-A- Read Begin");
    let readed_len = syscalls::read(fd_r, &mut buffer).unwrap();
    debug!("-A- Readed len: {}", readed_len);

    for (count, it) in buffer.iter().take(readed_len).enumerate() {
        assert_eq!(it, &(count as u8));
    }

    let code = syscalls::wait(pid).unwrap();
    assert_eq!(code, 0);

    let err = syscalls::read(fd_r, &mut buffer).unwrap_err();
    assert_eq!(err, SysError::OtherEndClosed);

    0
}

fn io_write_more(args: SpawnArgs) -> i8 {
    let (fd_r, fd_w) = syscalls::pipe().unwrap();

    let argv = ["hello".to_string(), "world".to_string()];
    let pid = args
        .new_spawn(&argv, &[fd_r, 0])
        .expect("run spawn base io");
    assert_eq!(pid, 1);

    let err = syscalls::write(fd_w + 20, &[0; 8]).unwrap_err();
    assert_eq!(err, SysError::InvalidFd);

    debug!("-A- Write --");
    let write_buf = alloc::vec![argv[0].as_bytes(), argv[1].as_bytes()].concat();
    let len = syscalls::write(fd_w, &write_buf).expect("write");
    debug!("-A- Write End --");
    assert_eq!(len, write_buf.len());

    let code = syscalls::wait(pid).unwrap();
    assert_eq!(code, 0);

    let err = syscalls::write(fd_w, &[0; 8]).unwrap_err();
    assert_eq!(err, SysError::OtherEndClosed);

    0
}
