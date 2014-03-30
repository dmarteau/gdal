use native;
use std::comm::channel;


struct WorkUnit {
    arg: int,
    callback: Sender<int>,
}


enum MessageToWorker {
    Work(WorkUnit),
    Halt,
}


enum MessageToDispatcher {
    Dispatch(WorkUnit),
    HaltAll,
    RegisterWorker(Sender<Sender<Sender<MessageToWorker>>>),
}


struct WorkQueue {
    dispatcher: Sender<MessageToDispatcher>,
}


impl WorkQueue {
    pub fn create() -> WorkQueue {
        let (dispatcher, dispatcher_inbox) = channel::<MessageToDispatcher>();

        // dispatcher
        native::task::spawn(proc() {
            let (want_work, idle_worker) = channel::<Sender<MessageToWorker>>();
            let mut worker_count = 0;
            let inbox = dispatcher_inbox;
            let idle_worker = idle_worker;
            loop {
                match inbox.recv() {
                    Dispatch(work_item) => {
                        idle_worker.recv().send(Work(work_item));
                    },
                    RegisterWorker(want_idle_sender) => {
                        worker_count += 1;
                        want_idle_sender.send(want_work.clone());
                    }
                    HaltAll => {
                        while worker_count > 0 {
                            idle_worker.recv().send(Halt);
                            worker_count -= 1;
                        }
                        return;
                    },
                };
            }
        });
        return WorkQueue{dispatcher: dispatcher};
    }

    pub fn register_worker(&self) -> Sender<Sender<MessageToWorker>> {
        let (reg_s, reg_r) = channel::<Sender<Sender<MessageToWorker>>>();
        self.dispatcher.send(RegisterWorker(reg_s));
        return reg_r.recv();
    }

    pub fn execute(&self, arg: int) -> Receiver<int> {
        let (callback, wait_for_rv) = channel::<int>();
        self.dispatcher.send(Dispatch(WorkUnit{arg: arg, callback: callback}));
        return wait_for_rv;
    }
}


impl Drop for WorkQueue {
    fn drop(&mut self) {
        self.dispatcher.send(HaltAll);
    }
}


#[test]
fn test_queue() {
    let queue = WorkQueue::create();
    for _ in range(0, 3) {
        let want_work = queue.register_worker();
        native::task::spawn(proc() {
            let want_work = want_work;
            loop {
                let (idle, get_work_unit) = channel::<MessageToWorker>();
                want_work.send(idle);
                let work_unit = match get_work_unit.recv() {
                    Work(wu) => wu,
                    Halt     => return
                };
                let rv = work_unit.arg * 2;
                work_unit.callback.send(rv);
            }
        });
    }
    let mut promise_list: ~[Receiver<int>] = ~[];
    for c in range(0, 10) {
        let rv = queue.execute(c);
        promise_list.push(rv);
    }
    let return_list = promise_list.map(|promise| promise.recv());
    assert_eq!(return_list, ~[0, 2, 4, 6, 8, 10, 12, 14, 16, 18]);
}