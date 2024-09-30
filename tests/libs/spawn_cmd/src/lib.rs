#![no_std]

pub enum SpawnCmd {
    Base,
    EmptyPipe,
    BaseIO1,
    BaseIO2,
    BaseIO3,
}
impl From<u8> for SpawnCmd {
    fn from(value: u8) -> Self {
        match value {
            0 => Self::Base,
            1 => Self::EmptyPipe,
            2 => Self::BaseIO1,
            3 => Self::BaseIO2,
            4 => Self::BaseIO3,
            _ => panic!("unknow value"),
        }
    }
}
impl Into<u8> for SpawnCmd {
    fn into(self) -> u8 {
        match self {
            Self::Base => 0,
            Self::EmptyPipe => 1,
            Self::BaseIO1 => 2,
            Self::BaseIO2 => 3,
            Self::BaseIO3 => 4,
        }
    }
}
impl From<&str> for SpawnCmd {
    fn from(value: &str) -> Self {
        u8::from_str_radix(value, 10).expect("parse cmd").into()
    }
}
