use crate::global_data::{TxID, VmID};
use std::{
    cell::RefCell,
    collections::HashMap,
    sync::{Arc, Condvar, Mutex},
};

thread_local! {
    static TX_CONTEXT_ID: RefCell<TxID> = RefCell::new(TxID::default());
    static VM_CONTEXT_ID: RefCell<VmID> = RefCell::new(VmID::default());
}

const MAX_VMS_COUNT: u64 = 16;

pub struct Child {
    id: VmID,
    inherited_fds: Vec<Fd>,

    event_wait: Event,
    event_notify: Event,
}

#[derive(Default)]
pub struct VMInfo {
    id: VmID,

    inherited_fds: Vec<Fd>,
    event_wait: Event,
    event_notify: Event,

    children: HashMap<VmID, Child>, // wait, notify
}
impl VMInfo {
    pub fn set_ctx_id(id: VmID) {
        VM_CONTEXT_ID.with(|f| *f.borrow_mut() = id);
    }
    pub fn ctx_id() -> VmID {
        VM_CONTEXT_ID.with(|f| f.borrow().clone())
    }

    pub fn inherited_fds(&self) -> Vec<Fd> {
        self.inherited_fds.clone()
    }

    fn get_child_by_fd(&self, fd: &Fd) -> Option<&Child> {
        if let Some((_, child)) = self
            .children
            .iter()
            .find(|(_id, child)| child.inherited_fds.iter().any(|f| f == fd))
        {
            Some(child)
        } else {
            None
        }
    }

    fn get_child_by_id(&self, id: &VmID) -> Option<&Child> {
        if let Some((_, c)) = self.children.iter().find(|(_, child)| &child.id == id) {
            Some(c)
        } else {
            None
        }
    }

    pub fn wait(&self, fd: Option<&Fd>) -> Event {
        if let Some(fd) = fd {
            if self.inherited_fds.iter().any(|f| f == fd) {
                self.event_wait.clone()
            } else if let Some(child) = self.get_child_by_fd(&fd.other_fd()) {
                child.event_wait.clone()
            } else {
                panic!("wait unknow fd {:?}", fd);
            }
        } else {
            self.event_wait.clone()
        }
    }
    pub fn notify(&self, fd: Option<&Fd>) {
        if let Some(fd) = fd {
            if self.inherited_fds.iter().any(|f| f == fd) {
                self.event_notify.notify();
            } else if let Some(child) = self.get_child_by_fd(&fd.other_fd()) {
                child.event_notify.notify();
            } else {
                panic!("notify unknow fd {:?}", fd);
            }
        } else {
            self.event_notify.notify();
        }
    }

    pub fn wait_by_pid(&self, id: &VmID) -> Event {
        if id == &self.id {
            self.event_wait.clone()
        } else if let Some(c) = self.get_child_by_id(id) {
            c.event_wait.clone()
        } else {
            panic!("notify unknow pid {:?}", id);
        }
    }
}

pub struct TxContext {
    fds_count: u64,
    vm_id_count: VmID,

    vm_info: HashMap<VmID, VMInfo>,

    fds: HashMap<Fd, VmID>,
    bufs: HashMap<Fd, Vec<u8>>,
}
impl Default for TxContext {
    fn default() -> Self {
        VMInfo::set_ctx_id(0.into());
        Self {
            fds_count: 2,
            vm_id_count: 1.into(),
            vm_info: [(0.into(), VMInfo::default())].into(),
            fds: Default::default(),
            bufs: Default::default(),
        }
    }
}
impl TxContext {
    pub fn set_ctx_id(id: TxID) {
        TX_CONTEXT_ID.with(|f| *f.borrow_mut() = id);
    }
    pub fn ctx_id() -> TxID {
        TX_CONTEXT_ID.with(|f| f.borrow().clone())
    }
    pub fn clean() {
        TX_CONTEXT_ID.with(|f| *f.borrow_mut() = 0.into());
        VM_CONTEXT_ID.with(|f| *f.borrow_mut() = 0.into());
    }

    pub fn new_vm(&mut self, parent_id: Option<VmID>, fds: &[Fd]) -> VmID {
        assert!(parent_id.is_none() == fds.is_empty());

        let id = self.vm_id_count.next();
        let (e_wait, e_notify) = if parent_id.is_some() {
            let p = self
                .vm_info
                .get_mut(parent_id.as_ref().unwrap())
                .unwrap_or_else(|| panic!("unknow pid: {:?}", parent_id));
            let e_wait = Event::default();
            let e_notify = Event::default();

            p.children.insert(
                id.clone(),
                Child {
                    id: id.clone(),
                    event_wait: e_wait.clone(),
                    event_notify: e_notify.clone(),
                    inherited_fds: fds.to_vec(),
                },
            );
            (e_wait, e_notify)
        } else {
            // TODO
            (Event::default(), Event::default())
        };

        let p = VMInfo {
            id: id.clone(),
            inherited_fds: fds.to_vec(),
            event_notify: e_wait,
            event_wait: e_notify,
            children: Default::default(),
        };

        self.vm_info.insert(id.clone(), p);

        id
    }
    pub fn vm_info(&self, id: &VmID) -> &VMInfo {
        self.vm_info
            .get(id)
            .unwrap_or_else(|| panic!("unknow vm id: {:?}", id))
    }
    pub fn max_vms_spawned(&self) -> bool {
        u64::from(self.vm_id_count.clone()) >= MAX_VMS_COUNT
    }

    pub fn new_pipe(&mut self) -> (Fd, Fd) {
        let pid = VMInfo::ctx_id();
        let fds = Fd::create(self.fds_count);

        self.fds.insert(fds.0.clone(), pid.clone());
        self.fds.insert(fds.1.clone(), pid.clone());
        self.fds_count = fds.2;

        (fds.0, fds.1)
    }
    pub fn close_pipe(&mut self, fd: Fd) -> bool {
        if !self.has_fd(&fd) {
            false
        } else {
            self.fds.remove(&fd).is_some()
        }
    }
    pub fn len_pipe(&self) -> usize {
        self.fds.len()
    }
    pub fn move_pipe(&mut self, fd: &Fd, pid: VmID) {
        let f = self
            .fds
            .get_mut(fd)
            .unwrap_or_else(|| panic!("unknow fd: {:?}", fd));
        *f = pid;
    }

    pub fn has_fd(&self, fd: &Fd) -> bool {
        if let Some(pid) = self.fds.get(fd) {
            &VMInfo::ctx_id() == pid
        } else {
            false
        }
    }
    pub fn chech_other_fd(&self, fd: &Fd) -> bool {
        self.fds.contains_key(&fd.other_fd())
    }

    pub fn read_data(&mut self, fd: &Fd, len: usize) -> (Vec<u8>, usize) {
        let data = self.bufs.get(fd);
        if data.is_none() {
            return (Vec::new(), 0);
        }
        let data = data.unwrap().clone();

        if len >= data.len() {
            self.bufs.remove(fd);
            (data, 0)
        } else {
            *self.bufs.get_mut(fd).unwrap() = data[len..].to_vec();
            (data[..len].to_vec(), data.len() - len)
        }
    }
    pub fn write_data(&mut self, fd: &Fd, buf: &[u8]) {
        if let Some(bufs) = self.bufs.get_mut(&fd.other_fd()) {
            bufs.extend_from_slice(buf);
        } else {
            self.bufs.insert(fd.other_fd(), buf.to_vec());
        }
    }
    pub fn has_data(&self, fd: &Fd) -> bool {
        self.bufs.contains_key(fd) || self.bufs.contains_key(&fd.other_fd())
    }
}

#[derive(Default, Debug)]
pub struct Event {
    data: Arc<(Mutex<bool>, Condvar)>,
}
impl Clone for Event {
    fn clone(&self) -> Self {
        Self {
            data: Arc::clone(&self.data),
        }
    }
}
impl Event {
    pub fn notify(&self) {
        let (lock, cvar) = &*self.data;
        let mut started = lock.lock().unwrap();
        *started = true;
        cvar.notify_one();
    }

    pub fn wait(&self) {
        let (lock, cvar) = &*self.data;
        let mut started = lock.lock().unwrap();

        loop {
            if *started {
                *started = false;
                break;
            }
            started = cvar.wait(started).unwrap();
        }
    }
}

#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub struct Fd(pub u64);
impl From<u64> for Fd {
    fn from(value: u64) -> Self {
        Self(value)
    }
}
impl From<Fd> for u64 {
    fn from(value: Fd) -> Self {
        value.0
    }
}
impl Fd {
    pub fn create(slot: u64) -> (Fd, Fd, u64) {
        (Fd(slot), Fd(slot + 1), slot + 2)
    }
    pub fn other_fd(&self) -> Fd {
        Fd(self.0 ^ 0x1)
    }
    pub fn is_read(&self) -> bool {
        self.0 % 2 == 0
    }
}
