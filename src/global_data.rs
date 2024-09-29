use crate::process_info::TxContext;
use std::{
    collections::HashMap,
    ffi::c_void,
    sync::{Mutex, MutexGuard},
};

lazy_static! {
    static ref GLOBAL_DATA: Mutex<GlobalData> = Default::default();
}
static mut GLOBAL_DATA_PTR: *mut Mutex<GlobalData> = std::ptr::null_mut();

#[derive(Default, PartialEq, Eq, Clone, Hash, Debug)]
pub struct TxID(u64);
impl From<u64> for TxID {
    fn from(value: u64) -> Self {
        Self(value)
    }
}
impl From<TxID> for u64 {
    fn from(value: TxID) -> Self {
        value.0
    }
}
impl TxID {
    fn next(&mut self) -> Self {
        self.0 += 1;
        self.clone()
    }
}

#[derive(Default, PartialEq, Eq, Clone, Hash, Debug)]
pub struct ProcID(u64);
impl From<u64> for ProcID {
    fn from(value: u64) -> Self {
        Self(value)
    }
}
impl From<ProcID> for u64 {
    fn from(value: ProcID) -> Self {
        value.0
    }
}
impl ProcID {
    pub fn next(&mut self) -> Self {
        let id = self.clone();
        self.0 += 1;
        id
    }
}

pub struct GlobalData {
    tx_ctx: HashMap<TxID, TxContext>,
    tx_ctx_id_count: TxID,
}
impl Default for GlobalData {
    fn default() -> Self {
        TxContext::set_ctx_id(0.into());
        Self {
            tx_ctx: [(0.into(), TxContext::default())].into(),
            tx_ctx_id_count: 1.into(),
        }
    }
}

impl GlobalData {
    pub fn get() -> &'static Mutex<Self> {
        if unsafe { GLOBAL_DATA_PTR.is_null() } {
            &GLOBAL_DATA
        } else {
            unsafe { &mut *GLOBAL_DATA_PTR as &mut Mutex<Self> }
        }
    }
    pub fn locked() -> MutexGuard<'static, Self> {
        Self::get().lock().unwrap()
    }
    pub fn get_ptr() -> *const c_void {
        if unsafe { GLOBAL_DATA_PTR.is_null() } {
            let infos_ref: &Mutex<Self> = &GLOBAL_DATA;
            infos_ref as *const Mutex<Self> as *const c_void
        } else {
            unsafe { GLOBAL_DATA_PTR as *const c_void }
        }
    }
    pub fn set_ptr(ptr: *const c_void) {
        unsafe {
            GLOBAL_DATA_PTR = ptr as *mut Mutex<GlobalData>;
        }
    }

    pub fn set_tx(&mut self, ctx: TxContext) -> TxID {
        self.tx_ctx.insert(self.tx_ctx_id_count.next(), ctx);
        self.tx_ctx_id_count.clone()
    }
    pub fn get_tx(&self, id: &TxID) -> &TxContext {
        self.tx_ctx
            .get(id)
            .unwrap_or_else(|| panic!("unknow tx context: {:?}", id))
    }
    pub fn get_tx_mut(&mut self, id: &TxID) -> &mut TxContext {
        self.tx_ctx
            .get_mut(id)
            .unwrap_or_else(|| panic!("unknow mut tx context: {:?}", id))
    }
}

#[macro_export]
macro_rules! get_tx {
    ($txid:expr) => {
        GlobalData::locked().get_tx(&$txid)
    };
}

#[macro_export]
macro_rules! get_tx_mut {
    ($txid:expr) => {
        GlobalData::locked().get_tx_mut(&$txid)
    };
}

#[macro_export]
macro_rules! get_proc {
    ($txid: expr, $procid: expr) => {
        GlobalData::locked().get_tx(&$txid).process(&$procid)
    };
}

#[macro_export]
macro_rules! get_cur_proc {
    () => {
        GlobalData::locked()
            .get_tx(&TxContext::ctx_id())
            .process(&Process::ctx_id())
    };
}
