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
use ckb_std::{debug, syscalls};
use spawn_cmd::SpawnCmd;

pub fn program_entry() -> i8 {
    debug!("-B- Spawn-Child(pid:{}) Begin --", syscalls::process_id());

    let argv = ckb_std::env::argv();
    assert!(!argv.is_empty(), "child args is failed: {}", argv.len());

    let argv: Vec<String> = argv
        .iter()
        .map(|f| f.to_str().unwrap().to_string())
        .collect();

    let cmd: SpawnCmd = argv[0].as_str().into();
    let argv = argv[1..].to_vec();

    let rc = cmd_routing(cmd, &argv);

    debug!("-B- Spawn-Child(pid:{}) End --", syscalls::process_id());
    rc
}

fn cmd_routing(cmd: SpawnCmd, argv: &[String]) -> i8 {
    debug!("-B- cmd: {:?}", cmd);
    match cmd {
        SpawnCmd::Base => spawn_base(argv),
        SpawnCmd::SpawnRetNot0 => spawn_ret_not0(argv),
        SpawnCmd::WaitRetNot0 => wait_ret_not0(argv),
        SpawnCmd::WaitInvalidPid => panic!("pass"),
        SpawnCmd::EmptyPipe => panic!("unsupport EmptyPipe"),
        SpawnCmd::SpawnInvalidFd => panic!("unsupport SpawnInvalidFd"),
        SpawnCmd::SpawnMaxVms => spawn_max_vms(argv),
        SpawnCmd::PipeMaxFds => panic!("pass"),
        SpawnCmd::BaseIO1 => spawn_base_io1(argv),
        SpawnCmd::BaseIO2 => spawn_base_io2(argv),
        SpawnCmd::BaseIO3 => spawn_base_io3(argv),
        SpawnCmd::BaseIO4 => spawn_base_io4(argv),
        SpawnCmd::IOReadMore => io_read_more(argv),
        SpawnCmd::IOWriteMore => io_write_more(argv),
    }
}

fn spawn_base(_argv: &[String]) -> i8 {
    let mut std_fds = [0u64; 2];
    syscalls::inherited_fds(&mut std_fds);
    assert_eq!(std_fds[0], 4);
    assert_eq!(std_fds[1], 3);

    let mut std_fds2 = [0u64; 3];
    syscalls::inherited_fds(&mut std_fds2);
    assert_eq!(std_fds2[0], 4);
    assert_eq!(std_fds2[1], 3);
    assert_eq!(std_fds2[2], 0);

    0
}

fn spawn_ret_not0(_argv: &[String]) -> i8 {
    3
}

fn wait_ret_not0(_argv: &[String]) -> i8 {
    let mut std_fds: [u64; 1] = [0; 1];
    syscalls::inherited_fds(&mut std_fds);

    let mut buf = [0u8; 32];
    let _ = syscalls::read(std_fds[0], &mut buf);

    2
}

fn spawn_max_vms(_argv: &[String]) -> i8 {
    let mut std_fds: [u64; 1] = [0; 1];
    syscalls::inherited_fds(&mut std_fds);

    let mut buf = [0u8; 32];
    syscalls::read(std_fds[0], &mut buf).expect("child write");

    0
}

fn spawn_base_io1(argv: &[String]) -> i8 {
    let mut std_fds: [u64; 2] = [0; 2];
    debug!("-B- InheritedFds --");
    syscalls::inherited_fds(&mut std_fds);
    debug!("-B- InheritedFds {} {} End --", std_fds[0], std_fds[1]);

    let mut buffer = vec![];
    for arg in argv {
        buffer.extend_from_slice(arg.as_bytes());
    }

    debug!(
        "-B- Write, fd: {}, buf_len({}) --",
        std_fds[1],
        buffer.len()
    );
    let len = syscalls::write(std_fds[1], &buffer).expect("child write");
    debug!("-B-    buf: {:02x?}", buffer);
    debug!("-B- Write End --");
    assert_eq!(len, 10);

    debug!(
        "-B- Write2, fd: {}, buf_len({}) --",
        std_fds[1],
        buffer.len()
    );
    let bufff = [0, 1, 2, 3, 4, 5, 6, 7, 8, 0, 1, 2, 3, 4, 5, 6, 7, 8];
    let len = syscalls::write(std_fds[1], &bufff).expect("child write");
    debug!("-B- Write2 End --");
    assert_eq!(len, 10);

    0
}

fn spawn_base_io2(argv: &[String]) -> i8 {
    let mut std_fds: [u64; 2] = [0; 2];
    debug!("-B- InheritedFds --");
    syscalls::inherited_fds(&mut std_fds);
    debug!("-B- InheritedFds {} {} End --", std_fds[0], std_fds[1]);

    let mut out = vec![];
    for arg in argv {
        out.extend_from_slice(arg.as_bytes());
    }

    debug!("-B- Read --");
    let mut buf: [u8; 256] = [0; 256];
    let len = syscalls::read(std_fds[0], &mut buf).expect("read 1");
    debug!("-B- Read End --");

    assert_eq!(len, out.len());
    assert_eq!(out, buf[..out.len()]);
    0
}

fn spawn_base_io3(argv: &[String]) -> i8 {
    let mut std_fds: [u64; 2] = [0; 2];
    debug!("-B- InheritedFds --");
    syscalls::inherited_fds(&mut std_fds);
    debug!("-B- InheritedFds {} {} End --", std_fds[0], std_fds[1]);

    let mut out = vec![];
    for arg in argv {
        out.extend_from_slice(arg.as_bytes());
    }

    debug!("-B- Read --");
    let mut buf: [u8; 256] = [0; 256];
    let len = syscalls::read(std_fds[0], &mut buf).expect("read 1");
    debug!("-B- Read End --");

    assert_eq!(len, out.len());
    assert_eq!(out, buf[..out.len()]);
    0
}

fn spawn_base_io4(argv: &[String]) -> i8 {
    let mut std_fds: [u64; 2] = [0; 2];
    syscalls::inherited_fds(&mut std_fds);

    let mut out = vec![];
    for arg in argv {
        out.extend_from_slice(arg.as_bytes());
    }

    debug!("-B- write: {:02x?}", out);
    let len = syscalls::write(std_fds[1], &out).expect("child write");
    assert_eq!(len, 10);
    0
}

fn io_read_more(_argv: &[String]) -> i8 {
    let fd_w = {
        let mut std_fds = [0; 1];
        syscalls::inherited_fds(&mut std_fds);
        std_fds[0]
    };

    let mut buffer1 = [0u8; 32];
    let mut count = 0;
    buffer1.iter_mut().all(|f| {
        *f = count;
        count += 1;
        true
    });
    syscalls::write(fd_w, &buffer1).unwrap();

    0
}

fn io_write_more(argv: &[String]) -> i8 {
    let fd_r = {
        let mut std_fds = [0; 1];
        syscalls::inherited_fds(&mut std_fds);
        std_fds[0]
    };

    let mut out = vec![];
    for arg in argv {
        out.extend_from_slice(arg.as_bytes());
    }

    debug!("-B- Read --");
    let mut buf: [u8; 256] = [0; 256];
    let len = syscalls::read(fd_r, &mut buf).expect("read 1");
    debug!("-B- Read End --");

    assert_eq!(len, out.len());
    assert_eq!(out, buf[..out.len()]);
    0
}
