use crate::global_data::{ProcID, TxID};
use std::{
    cell::RefCell,
    collections::HashMap,
    sync::{Arc, Condvar, Mutex},
};

thread_local! {
    static TX_CONTEXT_ID: RefCell<TxID> = RefCell::new(TxID::default());
    static PROCESS_CONTEXT_ID: RefCell<ProcID> = RefCell::new(ProcID::default());
}

pub struct Child {
    id: ProcID,
    inherited_fds: Vec<Fd>,

    event_wait: Event,
    event_notify: Event,
}

#[derive(Default)]
pub struct Process {
    id: ProcID,

    inherited_fds: Vec<Fd>,
    event_wait: Event,
    event_notify: Event,

    children: HashMap<ProcID, Child>, // wait, notify
}
impl Process {
    pub fn set_ctx_id(id: ProcID) {
        PROCESS_CONTEXT_ID.with(|f| *f.borrow_mut() = id);
    }
    pub fn ctx_id() -> ProcID {
        PROCESS_CONTEXT_ID.with(|f| f.borrow().clone())
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

    fn get_child_by_id(&self, id: &ProcID) -> Option<&Child> {
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

    pub fn wait_by_pid(&self, id: &ProcID) -> Event {
        if id == &self.id {
            self.event_wait.clone()
        } else {
            if let Some(c) = self.get_child_by_id(id) {
                c.event_wait.clone()
            } else {
                panic!("notify unknow pid {:?}", id);
            }
        }
    }
}

pub struct TxContext {
    fds_count: u64,
    proc_id_count: ProcID,

    proc_info: HashMap<ProcID, Process>,

    fds: HashMap<Fd, ProcID>,
    bufs: HashMap<Fd, Vec<u8>>,
}
impl Default for TxContext {
    fn default() -> Self {
        Process::set_ctx_id(0.into());
        Self {
            fds_count: 2,
            proc_id_count: 1.into(),
            proc_info: [(0.into(), Process::default())].into(),
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

    pub fn new_process(&mut self, parent_id: Option<ProcID>, fds: &[Fd]) -> ProcID {
        assert!(parent_id.is_none() == fds.is_empty());

        let id = self.proc_id_count.next();
        let (e_wait, e_notify) = if parent_id.is_some() {
            let p = self
                .proc_info
                .get_mut(parent_id.as_ref().unwrap())
                .expect(&format!("unknow pid: {:?}", parent_id));
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

        let p = Process {
            id: id.clone(),
            inherited_fds: fds.to_vec(),
            event_notify: e_wait,
            event_wait: e_notify,
            children: Default::default(),
        };

        self.proc_info.insert(id.clone(), p);

        id
    }
    pub fn process(&self, id: &ProcID) -> &Process {
        self.proc_info
            .get(id)
            .expect(&format!("unknow process id: {:?}", id))
    }

    pub fn new_pipe(&mut self) -> (Fd, Fd) {
        let pid = Process::ctx_id();
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
    pub fn move_pipe(&mut self, fd: &Fd, pid: ProcID) {
        let f = self.fds.get_mut(fd).expect(&format!("unknow fd: {:?}", fd));
        *f = pid;
    }

    pub fn has_fd(&self, fd: &Fd) -> bool {
        if let Some(pid) = self.fds.get(&fd) {
            &Process::ctx_id() == pid
        } else {
            false
        }
    }
    pub fn chech_other_fd(&self, fd: &Fd) -> bool {
        self.fds.get(&fd.other_fd()).is_some()
    }

    pub fn read_data(&mut self, fd: &Fd, len: usize) -> Vec<u8> {
        let data = self.bufs.get(&fd);
        if data.is_none() {
            return Vec::new();
        }
        let data = data.unwrap().clone();

        if len >= data.len() {
            self.bufs.remove(fd);
            data
        } else {
            *self.bufs.get_mut(fd).unwrap() = data[len..].to_vec();
            data[..len].to_vec()
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
        return self.bufs.get(fd).is_some() || self.bufs.get(&fd.other_fd()).is_some();
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
        let mut event = lock.lock().unwrap();

        loop {
            if *event {
                *event = false;
                break;
            }
            event = cvar.wait(event).unwrap();
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
impl Into<u64> for Fd {
    fn into(self) -> u64 {
        self.0
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
