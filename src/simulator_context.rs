use crate::utils::{Event, Fd, ProcID, SimID};
use std::{cell::RefCell, collections::HashMap, thread::JoinHandle};

thread_local! {
    static TX_CONTEXT_ID: RefCell<SimID> = RefCell::new(SimID::default());
    static PROC_CONTEXT_ID: RefCell<ProcID> = RefCell::new(ProcID::default());
}

const MAX_PROCESSES_COUNT: u64 = 16;

pub struct Child {
    id: ProcID,
    inherited_fds: Vec<Fd>,

    event_wait: Event,
    event_notify: Event,
}

#[derive(Default)]
pub struct ProcInfo {
    id: ProcID,

    inherited_fds: Vec<Fd>,
    event_wait: Event,
    event_notify: Event,

    children: HashMap<ProcID, Child>, // wait, notify

    join_handle: Option<JoinHandle<i8>>,
    wait_exit: bool,
}
impl ProcInfo {
    pub fn set_ctx_id(id: ProcID) {
        PROC_CONTEXT_ID.with(|f| *f.borrow_mut() = id);
    }
    pub fn ctx_id() -> ProcID {
        PROC_CONTEXT_ID.with(|f| f.borrow().clone())
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

    pub fn get_event_by_pid(&self, id: &ProcID) -> Event {
        if id == &self.id {
            self.event_wait.clone()
        } else if let Some(c) = self.get_child_by_id(id) {
            c.event_wait.clone()
        } else {
            panic!("notify unknow pid {:?}", id);
        }
    }

    pub fn set_join(&mut self, j: JoinHandle<i8>) {
        self.join_handle = Some(j);
    }

    pub fn wait_exit(&mut self) -> Option<JoinHandle<i8>> {
        self.event_notify.notify();
        self.event_wait.notify();
        self.wait_exit = true;

        self.join_handle.take()
    }
}

pub struct SimContext {
    fds_count: u64,
    proc_id_count: ProcID,

    proc_info: HashMap<ProcID, ProcInfo>,

    fds: HashMap<Fd, ProcID>,
    bufs: HashMap<Fd, Vec<u8>>,
}
impl Default for SimContext {
    fn default() -> Self {
        ProcInfo::set_ctx_id(0.into());
        Self {
            fds_count: 2,
            proc_id_count: 1.into(),
            proc_info: [(0.into(), ProcInfo::default())].into(),
            fds: Default::default(),
            bufs: Default::default(),
        }
    }
}
impl SimContext {
    pub fn set_ctx_id(id: SimID) {
        TX_CONTEXT_ID.with(|f| *f.borrow_mut() = id);
    }
    pub fn ctx_id() -> SimID {
        TX_CONTEXT_ID.with(|f| f.borrow().clone())
    }
    pub fn clean() {
        TX_CONTEXT_ID.with(|f| *f.borrow_mut() = 0.into());
        PROC_CONTEXT_ID.with(|f| *f.borrow_mut() = 0.into());
    }

    pub fn new_process(&mut self, parent_id: Option<ProcID>, fds: &[Fd]) -> ProcID {
        let id = self.proc_id_count.next();
        let (e_wait, e_notify) = if parent_id.is_some() {
            let p = self
                .proc_info
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

        let proc = ProcInfo {
            id: id.clone(),
            inherited_fds: fds.to_vec(),
            event_notify: e_wait,
            event_wait: e_notify,
            children: Default::default(),
            join_handle: None,
            wait_exit: false,
        };

        self.proc_info.insert(id.clone(), proc);

        id
    }
    pub fn proc_info(&self, id: &ProcID) -> &ProcInfo {
        self.proc_info
            .get(id)
            .unwrap_or_else(|| panic!("unknow process id: {:?}", id))
    }
    pub fn proc_mut_info(&mut self, id: &ProcID) -> &mut ProcInfo {
        self.proc_info
            .get_mut(id)
            .unwrap_or_else(|| panic!("unknow process id: {:?}", id))
    }
    pub fn max_proc_spawned(&self) -> bool {
        u64::from(self.proc_id_count.clone()) > MAX_PROCESSES_COUNT
    }
    pub fn has_proc(&self, id: &ProcID) -> bool {
        self.proc_info.contains_key(id)
    }

    pub fn new_pipe(&mut self) -> (Fd, Fd) {
        let pid = ProcInfo::ctx_id();
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
        let f = self
            .fds
            .get_mut(fd)
            .unwrap_or_else(|| panic!("unknow fd: {:?}", fd));
        *f = pid;
    }
    pub fn close_all(&mut self, id: &ProcID) {
        let keys_to_rm: Vec<Fd> = self
            .fds
            .iter()
            .filter(|(_k, v)| v == &id)
            .map(|(k, _v)| k.clone())
            .collect();
        for k in keys_to_rm {
            self.fds.remove(&k);
        }
    }

    pub fn has_fd(&self, fd: &Fd) -> bool {
        if let Some(pid) = self.fds.get(fd) {
            &ProcInfo::ctx_id() == pid
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
