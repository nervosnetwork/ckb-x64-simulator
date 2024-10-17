#![no_std]

#[macro_use]
extern crate num_derive;

use num_traits::{FromPrimitive, ToPrimitive};

#[repr(u8)]
#[derive(FromPrimitive, ToPrimitive, Clone, Debug)]
pub enum SpawnCmd {
    Base = 1,
    SpawnRetNot0,
    WaitRetNot0,
    WaitInvalidPid,
    EmptyPipe,
    SpawnInvalidFd,
    SpawnMaxVms,
    PipeMaxFds,
    BaseIO1,
    BaseIO2,
    BaseIO3,
    BaseIO4,
    IOReadMore,
    IOWriteMore,
}

impl From<u8> for SpawnCmd {
    fn from(value: u8) -> Self {
        Self::from_u8(value).unwrap()
    }
}
impl From<&str> for SpawnCmd {
    fn from(value: &str) -> Self {
        value.parse::<u8>().unwrap().into()
    }
}
impl From<SpawnCmd> for u8 {
    fn from(value: SpawnCmd) -> Self {
        value.to_u8().unwrap()
    }
}

#[repr(u8)]
#[derive(FromPrimitive, ToPrimitive, Clone, Debug)]
pub enum SpawnCasesCmd {
    Unknow = 0,
    ReadWrite,
    WriteDeadLock,
    InvalidFd,
    WaitDeadLock,
    ReadWriteWithClose,
    WaitMultiple,
    InheritedFds,
    InheritedFdsWithoutOwner,
    ReadThenClose,
    MaxVmsCount,
    MaxFdsLimit,
    CloseInvalidFd,
    WriteClosedFd,
    CheckPID,
}
impl From<u8> for SpawnCasesCmd {
    fn from(value: u8) -> Self {
        Self::from_u8(value).unwrap()
    }
}
impl From<&str> for SpawnCasesCmd {
    fn from(value: &str) -> Self {
        value.parse::<u8>().unwrap().into()
    }
}
impl From<SpawnCasesCmd> for u8 {
    fn from(value: SpawnCasesCmd) -> Self {
        value.to_u8().unwrap()
    }
}
