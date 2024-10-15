use crate::simulator_context::{TxContext, TxID, VmID};
use std::{
    collections::HashMap,
    ffi::c_void,
    pin::Pin,
    sync::{Mutex, MutexGuard},
};

lazy_static! {
    static ref GLOBAL_DATA: Pin<Box<Mutex<GlobalData>>> = Pin::new(Box::default());
}
static mut GLOBAL_DATA_PTR: *mut Mutex<GlobalData> = std::ptr::null_mut();

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

    pub fn clean() {
        unsafe {
            GLOBAL_DATA_PTR = std::ptr::null_mut();
        }
        let mut data = Self::locked();
        *data = Self::default();
        TxContext::clean();
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
macro_rules! get_cur_tx {
    () => {
        GlobalData::locked().get_tx(&TxContext::ctx_id())
    };
}

#[macro_export]
macro_rules! get_tx_mut {
    ($txid:expr) => {
        GlobalData::locked().get_tx_mut(&$txid)
    };
}

#[macro_export]
macro_rules! get_cur_tx_mut {
    () => {
        GlobalData::locked().get_tx_mut(&TxContext::ctx_id())
    };
}

#[macro_export]
macro_rules! get_vm {
    ($txid: expr, $vm_id: expr) => {
        GlobalData::locked().get_tx(&$txid).vm_info(&$vm_id)
    };
}

#[macro_export]
macro_rules! get_cur_vm {
    () => {
        GlobalData::locked()
            .get_tx(&TxContext::ctx_id())
            .vm_info(&VMInfo::ctx_id())
    };
}

#[macro_export]
macro_rules! get_cur_vm_mut {
    () => {
        GlobalData::locked()
            .get_tx_mut(&TxContext::ctx_id())
            .vm_mut_info(&VMInfo::ctx_id())
    };
}
