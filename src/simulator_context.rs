use crate::{
    global_data::GlobalData,
    utils::{Event, Fd, ProcID, SimID},
};
use std::{cell::RefCell, collections::HashMap, thread::JoinHandle};

thread_local! {
    static TX_CONTEXT_ID: RefCell<SimID> = RefCell::new(SimID::default());
    static PROC_CONTEXT_ID: RefCell<ProcID> = RefCell::new(ProcID::default());
}

const MAX_PROCESSES_COUNT: u64 = 16;

#[derive(PartialEq, Eq, Clone)]
pub enum ProcStatus {
    Default(ProcID),
    WaitSpawn(ProcID),
    ReadWait(ProcID, Fd, usize, Vec<u8>, u64),
    WriteWait(ProcID, Fd, Vec<u8>, u64),
    CloseWait(ProcID, Fd),
    Terminated(ProcID),
}
impl Default for ProcStatus {
    fn default() -> Self {
        Self::Default(ProcInfo::id())
    }
}
impl std::fmt::Debug for ProcStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Default(pid) => write!(f, "Loaded({})", u64::from(pid.clone())),
            Self::WaitSpawn(pid) => write!(f, "WaitSpawn({})", u64::from(pid.clone())),
            Self::ReadWait(pid, fd, len, buf, dbg_id) => {
                if len == &0 {
                    write!(
                        f,
                        "ReadWait N(id: {}, pid:{}, fd: {}, bl: {})",
                        dbg_id,
                        u64::from(pid.clone()),
                        u64::from(fd.clone()),
                        buf.len()
                    )
                } else {
                    write!(
                        f,
                        "ReadWait(id: {}, pid:{}, fd: {}, nl: {}, bl: {})",
                        dbg_id,
                        u64::from(pid.clone()),
                        u64::from(fd.clone()),
                        len,
                        buf.len()
                    )
                }
            }
            Self::WriteWait(pid, fd, buf, dbg_id) => {
                if buf.is_empty() {
                    write!(
                        f,
                        "WriteWait N(id: {}, pid: {}, fd: {})",
                        dbg_id,
                        u64::from(pid.clone()),
                        u64::from(fd.clone()),
                    )
                } else {
                    write!(
                        f,
                        "WriteWait(id: {}, pid: {}, fd: {}, l: {})",
                        dbg_id,
                        u64::from(pid.clone()),
                        u64::from(fd.clone()),
                        buf.len()
                    )
                }
            }
            Self::CloseWait(pid, fd) => {
                write!(
                    f,
                    "Close(pid: {}, fd: {})",
                    u64::from(pid.clone()),
                    u64::from(fd.clone())
                )
            }
            Self::Terminated(pid) => write!(f, "Terminated({})", u64::from(pid.clone())),
        }
    }
}
impl ProcStatus {
    fn read_wait(&self) -> Option<(&ProcID, &Fd, &usize, &[u8])> {
        if let Self::ReadWait(pid, fd, len, buf, _) = self {
            Some((pid, fd, len, buf))
        } else {
            None
        }
    }
    fn read_wait_mut(&mut self) -> Option<(&ProcID, &mut Fd, &mut usize, &mut Vec<u8>)> {
        if let Self::ReadWait(pid, fd, len, buf, _) = self {
            Some((pid, fd, len, buf))
        } else {
            None
        }
    }
    fn write_wait(&self) -> Option<(&ProcID, &Fd, &[u8])> {
        if let Self::WriteWait(pid, fd, buf, _) = self {
            Some((pid, fd, buf))
        } else {
            None
        }
    }
    fn write_wait_mut(&mut self) -> Option<(&ProcID, &mut Fd, &mut Vec<u8>)> {
        if let Self::WriteWait(pid, fd, buf, _) = self {
            Some((pid, fd, buf))
        } else {
            None
        }
    }
}

#[derive(Default)]
struct ProcInfo {
    parent_id: ProcID,

    inherited_fds: Vec<Fd>,

    scheduler_event: Event,
    join_handle: Option<JoinHandle<i8>>,
}
impl ProcInfo {
    fn set_pid(id: ProcID) {
        PROC_CONTEXT_ID.with(|f| *f.borrow_mut() = id);
    }
    pub fn id() -> ProcID {
        PROC_CONTEXT_ID.with(|f| f.borrow().clone())
    }
}

pub struct SimContext {
    fd_count: u64,
    process_id_count: ProcID,

    processes: HashMap<ProcID, ProcInfo>,
    process_status: Vec<ProcStatus>,
    readed_cache: HashMap<Fd, Vec<u8>>,

    fds: HashMap<Fd, ProcID>,

    dbg_status_count: u64,
}
impl Default for SimContext {
    fn default() -> Self {
        ProcInfo::set_pid(0.into());
        Self {
            fd_count: 2,
            process_id_count: 1.into(),
            processes: [(0.into(), ProcInfo::default())].into(),
            process_status: Default::default(),
            readed_cache: Default::default(),
            fds: Default::default(),

            dbg_status_count: 0,
        }
    }
}
impl SimContext {
    pub fn update_ctx_id(id: SimID, pid: Option<ProcID>) {
        TX_CONTEXT_ID.with(|f| *f.borrow_mut() = id);
        if let Some(pid) = pid {
            ProcInfo::set_pid(pid);
        }
    }
    pub fn ctx_id() -> SimID {
        TX_CONTEXT_ID.with(|f| f.borrow().clone())
    }
    pub fn clean() {
        TX_CONTEXT_ID.with(|f| *f.borrow_mut() = 0.into());
        PROC_CONTEXT_ID.with(|f| *f.borrow_mut() = 0.into());
    }

    pub fn start_process<F: Send + 'static + FnOnce(SimID, ProcID) -> i8>(
        &mut self,
        fds: &[Fd],
        func: F,
    ) -> ProcID {
        let parent_id = ProcInfo::id();
        let id = self.process_id_count.next();
        let process = ProcInfo {
            parent_id: parent_id.clone(),
            inherited_fds: fds.to_vec(),
            ..Default::default()
        };

        self.processes.insert(id.clone(), process);
        let ctx_id = SimContext::ctx_id();

        fds.iter().all(|fd| {
            self.move_pipe(fd, id.clone());
            true
        });
        self.process_status.push(ProcStatus::WaitSpawn(id.clone()));

        let id2 = id.clone();
        let join_handle = std::thread::spawn(move || {
            SimContext::update_ctx_id(ctx_id.clone(), Some(id.clone()));
            let code = func(ctx_id.clone(), id.clone());

            let mut gd = GlobalData::locked();
            let cur_sim = gd.get_tx_mut(&SimContext::ctx_id());
            cur_sim.close_all(&id);
            cur_sim.process_io(None);

            code
        });

        self.process_mut(&id2).join_handle = Some(join_handle);

        id2
    }
    pub fn pid() -> ProcID {
        ProcInfo::id()
    }
    pub fn inherited_fds(&self) -> Vec<Fd> {
        let process = self.process(&ProcInfo::id());
        process.inherited_fds.clone()
    }
    fn process(&self, id: &ProcID) -> &ProcInfo {
        self.processes
            .get(id)
            .unwrap_or_else(|| panic!("unknow process id: {:?}", id))
    }
    fn process_mut(&mut self, id: &ProcID) -> &mut ProcInfo {
        self.processes
            .get_mut(id)
            .unwrap_or_else(|| panic!("unknow process id: {:?}", id))
    }
    pub fn max_proc_spawned(&self) -> bool {
        u64::from(self.process_id_count.clone()) > MAX_PROCESSES_COUNT
    }
    pub fn has_proc(&self, id: &ProcID) -> bool {
        self.processes.contains_key(id)
    }
    pub fn get_event(&self) -> Event {
        self.process(&ProcInfo::id()).scheduler_event.clone()
    }
    pub fn exit(&mut self, id: &ProcID) -> Option<JoinHandle<i8>> {
        self.process_io(None);

        let process = self.process_mut(id);

        process.join_handle.take()
    }

    fn process_io(&mut self, fd: Option<&Fd>) {
        println!("==status 1: {:?}", self.process_status);

        let mut update_rw = Vec::<(usize, usize, bool)>::new(); // Vec<(Read, Write)>
        for i in 0..self.process_status.len() {
            if let Some((_pid, rfd, rlen, _rbuf)) = self.process_status[i].read_wait() {
                if rlen != &0 {
                    assert!(rfd.is_read());
                    let write_fd = rfd.other_fd();

                    let mut is_close = false;
                    if let Some(w_pos) =
                        self.process_status.iter().position(|status| match status {
                            ProcStatus::WriteWait(_wpid, wfd, _wbuf, _) => wfd == &write_fd,
                            ProcStatus::CloseWait(_wpid, cfd) => {
                                is_close = true;
                                cfd == &write_fd
                            }
                            _ => false,
                        })
                    {
                        update_rw.push((i, w_pos, is_close));
                    }
                }
            }
        }
        update_rw.iter().for_each(|(r_pos, w_pos, is_close)| {
            if *is_close {
                let (_, _rfd, rlen, _rbuf) = self.process_status[*r_pos]
                    .read_wait_mut()
                    .expect("Unknow error");
                *rlen = 0;
            } else {
                let wbuf = self.process_status[*w_pos]
                    .write_wait()
                    .map(|(_, _, buf)| buf.to_vec())
                    .expect("unknow error");

                // Update Read Status
                let (_, _rfd, rlen, rbuf) = self.process_status[*r_pos]
                    .read_wait_mut()
                    .expect("Unknow error");
                let copy_len = (*rlen).min(wbuf.len());
                rbuf.extend_from_slice(&wbuf[..copy_len]);
                *rlen -= copy_len;

                // Update Write Status
                let (_, _wfd, wbuf) = self.process_status[*w_pos]
                    .write_wait_mut()
                    .expect("unknow error");
                *wbuf = wbuf[copy_len..].to_vec();
            }
        });

        println!("==status 2: {:?}", self.process_status);
        self.notify_status(fd);
        println!("==status 3: {:?}", self.process_status);
    }
    fn notify_status(&mut self, fd: Option<&Fd>) {
        if let Some(pos) = self.process_status.iter().position(|s| {
            if let Some((_pid, rfd, len, _)) = s.read_wait() {
                len == &0 && (fd == Some(rfd) || fd == Some(&rfd.other_fd()))
            } else {
                false
            }
        }) {
            if let Some((pid, fd, _len, buf)) = self.process_status.remove(pos).read_wait() {
                assert_eq!(pid, self.fds.get(fd).unwrap());
                self.process(pid).scheduler_event.notify();
                self.readed_cache.insert(fd.clone(), buf.to_vec());
            } else {
                panic!("unknow error");
            }
            return;
        }

        let mut notify_items = std::collections::BTreeMap::<ProcID, usize>::new();
        for i in 0..self.process_status.len() {
            match &self.process_status[i] {
                ProcStatus::Default(_) => (),
                ProcStatus::WaitSpawn(pid) => {
                    notify_items.insert(pid.clone(), i);
                }
                ProcStatus::ReadWait(pid, _fd, len, _buf, _) => {
                    if len == &0 {
                        notify_items.insert(pid.clone(), i);
                    }
                }
                ProcStatus::WriteWait(pid, _fd, buf, _) => {
                    if buf.is_empty() {
                        notify_items.insert(pid.clone(), i);
                    }
                }
                ProcStatus::CloseWait(pid, _fd) => {
                    notify_items.insert(pid.clone(), i);
                }
                ProcStatus::Terminated(pid) => {
                    notify_items.insert(pid.clone(), i);
                }
            };
        }
        if let Some((_pid, index)) = notify_items.pop_first() {
            match &self.process_status[index] {
                ProcStatus::Default(_) => (),
                ProcStatus::WaitSpawn(pid) => {
                    self.process(&self.process(pid).parent_id)
                        .scheduler_event
                        .notify();
                }
                ProcStatus::ReadWait(_, fd, _len, buf, _) => {
                    self.readed_cache.insert(fd.clone(), buf.clone());
                    let pid = self.fds.get(fd).expect("unknow error");
                    self.process(pid).scheduler_event.notify();
                }
                ProcStatus::WriteWait(_, fd, _buf, _) => {
                    let pid = self.fds.get(fd).expect("unknow error");
                    self.process(pid).scheduler_event.notify();
                }
                ProcStatus::CloseWait(pid, _) => {
                    self.process(&self.process(pid).parent_id)
                        .scheduler_event
                        .notify();
                }
                ProcStatus::Terminated(pid) => {
                    self.process(&self.process(pid).parent_id)
                        .scheduler_event
                        .notify();
                }
            };
            self.process_status.remove(index);
            return;
        }
        // let mut rm_index = None;
        // for i in (0..self.process_status.len()).rev() {
        //     let status = &self.process_status[i];
        //     match status {
        //         ProcStatus::Default(_) => (),
        //         ProcStatus::WaitSpawn(pid) => {
        //             self.process(&self.process(&pid).parent_id)
        //                 .scheduler_event
        //                 .notify();
        //             break;
        //         }
        //         ProcStatus::ReadWait(_, fd, len, buf, _) => {
        //             if len == &0 {
        //                 self.readed_cache.insert(fd.clone(), buf.clone());
        //                 let pid = self.fds.get(&fd).expect("unknow error");
        //                 self.process(pid).scheduler_event.notify();
        //                 rm_index = Some(i);
        //                 break;
        //             }
        //         }
        //         ProcStatus::WriteWait(_, fd, buf, _) => {
        //             if buf.is_empty() {
        //                 let pid = self.fds.get(&fd).expect("unknow error");
        //                 self.process(pid).scheduler_event.notify();
        //                 rm_index = Some(i);
        //                 break;
        //             }
        //         }
        //         ProcStatus::Terminated(pid) => {
        //             self.process(&self.process(&pid).parent_id)
        //                 .scheduler_event
        //                 .notify();
        //             rm_index = Some(i);
        //             break;
        //         }
        //     };
        // }
        // if let Some(index) = rm_index {
        //     self.process_status.remove(index);
        // }
    }
    pub fn wait_read(&mut self, fd: Fd, len: usize) -> Event {
        let id = ProcInfo::id();
        let dbg_id = self.dbg_status_count;
        self.dbg_status_count += 1;

        self.process_status.push(ProcStatus::ReadWait(
            id,
            fd.clone(),
            len,
            Vec::new(),
            dbg_id,
        ));
        self.process_io(Some(&fd));
        self.get_event()
    }
    pub fn wait_write(&mut self, fd: Fd, buf: &[u8]) -> Event {
        let id = ProcInfo::id();
        let dbg_id = self.dbg_status_count;
        self.dbg_status_count += 1;
        self.process_status
            .push(ProcStatus::WriteWait(id, fd.clone(), buf.to_vec(), dbg_id));

        self.process_io(Some(&fd));
        self.get_event()
    }
    pub fn read_cache(&mut self, fd: &Fd) -> Vec<u8> {
        if let Some(buf) = self.readed_cache.remove(fd) {
            buf
        } else {
            let mut r_buf = Vec::new();
            self.process_status.iter_mut().any(|status| {
                if let Some((_pid, rfd, _len, buf)) = status.read_wait_mut() {
                    if fd == rfd {
                        r_buf = buf.clone();
                        buf.clear();
                        true
                    } else {
                        false
                    }
                } else {
                    false
                }
            });
            r_buf
        }
    }

    pub fn new_pipe(&mut self) -> (Fd, Fd) {
        let pid = ProcInfo::id();
        let fds = Fd::create(self.fd_count);

        self.fds.insert(fds.0.clone(), pid.clone());
        self.fds.insert(fds.1.clone(), pid.clone());
        self.fd_count = fds.2;

        (fds.0, fds.1)
    }
    pub fn close_pipe(&mut self, fd: Fd) -> Result<Event, ()> {
        if !self.has_fd(&fd) {
            Err(())
        } else {
            self.process_status
                .push(ProcStatus::CloseWait(ProcInfo::id(), fd.clone()));
            self.process_io(Some(&fd));

            if self.fds.remove(&fd).is_some() {
                Ok(self.process(&ProcInfo::id()).scheduler_event.clone())
            } else {
                //
                Err(())
            }
        }
    }
    pub fn len_pipe(&self) -> usize {
        self.fds.len()
    }
    fn move_pipe(&mut self, fd: &Fd, pid: ProcID) {
        let f = self
            .fds
            .get_mut(fd)
            .unwrap_or_else(|| panic!("unknow fd: {:?}", fd));
        *f = pid;
    }
    fn close_all(&mut self, id: &ProcID) {
        let keys_to_rm: Vec<Fd> = self
            .fds
            .iter()
            .filter(|(_k, v)| v == &id)
            .map(|(k, _v)| k.clone())
            .collect();
        for k in keys_to_rm {
            self.fds.remove(&k);
        }

        self.process_status.push(ProcStatus::Terminated(id.clone()));
    }

    pub fn has_fd(&self, fd: &Fd) -> bool {
        if let Some(pid) = self.fds.get(fd) {
            &ProcInfo::id() == pid
        } else {
            false
        }
    }
    pub fn chech_other_fd(&self, fd: &Fd) -> bool {
        self.fds.contains_key(&fd.other_fd())
    }
}
