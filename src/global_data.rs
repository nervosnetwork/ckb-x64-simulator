use crate::{simulator_context::SimContext, utils::SimID};
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
    tx_ctx: HashMap<SimID, SimContext>,
    tx_ctx_id_count: SimID,
}
impl Default for GlobalData {
    fn default() -> Self {
        SimContext::update_ctx_id(0.into(), None);
        Self {
            tx_ctx: [(0.into(), SimContext::default())].into(),
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
        SimContext::clean();
    }

    pub fn set_tx(&mut self, ctx: SimContext) -> SimID {
        self.tx_ctx.insert(self.tx_ctx_id_count.next(), ctx);
        self.tx_ctx_id_count.clone()
    }
    pub fn get_tx(&self, id: &SimID) -> &SimContext {
        self.tx_ctx
            .get(id)
            .unwrap_or_else(|| panic!("unknow tx context: {:?}", id))
    }
    pub fn get_tx_mut(&mut self, id: &SimID) -> &mut SimContext {
        self.tx_ctx
            .get_mut(id)
            .unwrap_or_else(|| panic!("unknow mut tx context: {:?}", id))
    }
}

#[macro_export]
macro_rules! get_cur_tx {
    () => {
        GlobalData::locked().get_tx(&SimContext::ctx_id())
    };
}

#[macro_export]
macro_rules! get_cur_tx_mut {
    () => {
        GlobalData::locked().get_tx_mut(&SimContext::ctx_id())
    };
}
