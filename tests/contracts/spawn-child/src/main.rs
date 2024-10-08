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
    assert!(argv.len() >= 1, "child args is failed: {}", argv.len());

    let argv: Vec<String> = argv
        .into_iter()
        .map(|f| f.to_str().unwrap().to_string())
        .collect();

    let cmd: SpawnCmd = argv[0].as_str().into();
    let argv = argv[1..].to_vec();

    let rc = match cmd {
        SpawnCmd::Base => spawn_base(),
        SpawnCmd::EmptyPipe => panic!("unsupport EmptyPipe"),
        SpawnCmd::SpawnInvalidFd => panic!("unsupport SpawnInvalidFd"),
        SpawnCmd::BaseIO1 => spawn_base_io1(&argv),
        SpawnCmd::BaseIO2 => spawn_base_io2(&argv),
        SpawnCmd::BaseIO3 => spawn_base_io3(&argv),
        SpawnCmd::BaseIO4 => spawn_base_io4(&argv),
    };

    debug!("-B- Spawn-Child(pid:{}) End --", syscalls::process_id());
    rc
}

fn spawn_base() -> i8 {
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

fn spawn_base_io1(argv: &[String]) -> i8 {
    let mut std_fds: [u64; 2] = [0; 2];
    debug!("-B- InheritedFds --");
    syscalls::inherited_fds(&mut std_fds);
    debug!("-B- InheritedFds {} {} End --", std_fds[0], std_fds[1]);

    let mut out = vec![];
    for arg in argv {
        out.extend_from_slice(arg.as_bytes());
    }

    debug!("-B- Write, fd: {}, out({}) --", std_fds[1], out.len());
    let len = syscalls::write(std_fds[1], &out).expect("child write");
    debug!("-B-    out: {:02x?}", out);
    debug!("-B- Write End --");
    assert_eq!(len, 10);

    debug!("-B- Write2, fd: {}, out({}) --", std_fds[1], out.len());
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
