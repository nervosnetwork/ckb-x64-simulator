#![no_std]

#[macro_use]
extern crate num_derive;

#[repr(u8)]
#[derive(FromPrimitive, ToPrimitive)]
pub enum SpawnCmd {
    Base = 1,
    EmptyPipe,
    BaseIO1,
    BaseIO2,
    BaseIO3,
    BaseIO4,
}

use num_traits::{FromPrimitive, ToPrimitive};

impl From<u8> for SpawnCmd {
    fn from(value: u8) -> Self {
        Self::from_u8(value).unwrap()
    }
}

impl From<&str> for SpawnCmd {
    fn from(value: &str) -> Self {
        Self::from_u8(u8::from_str_radix(value, 10).expect("parse cmd")).unwrap()
    }
}

impl From<SpawnCmd> for u8 {
    fn from(value: SpawnCmd) -> Self {
        value.to_u8().unwrap()
    }
}
