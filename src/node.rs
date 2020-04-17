use petgraph::graph::{Graph, NodeIndex};
use petgraph::visit::EdgeRef;
use petgraph::Undirected;
use std::clone::Clone;
use std::cmp::PartialEq;
use std::collections::HashMap;
use std::marker::Copy;
use std::sync::atomic::AtomicBool;
use std::sync::mpsc::Sender;
use std::sync::{Arc, RwLock};

#[derive(PartialEq, Copy, Clone, Debug)]
pub enum State {
    Sleep,
    Find,
    Found,
}
#[derive(PartialEq, Clone, Debug)]
pub enum Message {
    Connect(u32, NodeIndex),              /* level */
    Initiate(u32, i32, State, NodeIndex), /* level, name, state */
    Test(u32, i32, NodeIndex),            /* level, name */
    Accept(NodeIndex),
    Reject(NodeIndex),
    Report(i32, NodeIndex), /* best_wt */
    ChangeRoot(NodeIndex),
}
#[derive(PartialEq, Copy, Clone)]
pub enum Status {
    Basic,
    Branch,
    Reject,
}
pub struct Node {
    index: NodeIndex,
    state: State,
    pub status: HashMap<NodeIndex, Status>,
    name: i32,
    level: u32,
    parent: Option<NodeIndex>,
    best_wt: i32,
    best_node: Option<NodeIndex>,
    rec: u32,
    test_node: Option<NodeIndex>,
    graph: Arc<RwLock<Graph<i32, i32, Undirected>>>,
    pub stop: Arc<RwLock<AtomicBool>>,
}

impl Node {
    pub fn new(
        graph: Arc<RwLock<Graph<i32, i32, Undirected>>>,
        index: NodeIndex,
        stop: Arc<RwLock<AtomicBool>>,
    ) -> Self {
        Node {
            index,
            state: State::Sleep,
            status: HashMap::new(),
            name: index.index() as i32,
            level: 0,
            parent: None,
            best_wt: std::i32::MAX,
            best_node: None,
            rec: 0,
            test_node: None,
            graph,
            stop,
        }
    }
    pub fn initialize(&mut self, sender_mapping: &HashMap<NodeIndex, Sender<Message>>) {
        //println!("Initializing node {:?}..", self.index);
        let graph = self.graph.read().expect("Error while reading 'graph':");
        let edges = graph.edges(self.index);
        let edge_min = edges
            .min_by_key(|edge_ref| edge_ref.weight())
            .expect("Error while finding least weight edge during initialization:");
        let src = edge_min.source();
        let target = edge_min.target();
        let nbr_q = if src == self.index { target } else { src };
        self.status.insert(nbr_q, Status::Branch);
        /* Filling up 'status' of every node other than 'nbr_q' to be 'Status::Basic' */
        for node_index in graph.node_indices() {
            if node_index != nbr_q {
                self.status.insert(node_index, Status::Basic);
            }
        }
        self.level = 0;
        self.state = State::Found;
        self.rec = 0;
        let sender = sender_mapping
            .get(&nbr_q)
            .expect("Error while reading 'sender_mapping':");
        let msg = Message::Connect(0, self.index);
        sender
            .send(msg.clone())
            .expect("Error while sending message:");
        /*println!(
            "Thread [{:?}]: Sent message {:?} to {:?}",
            self.index, msg, nbr_q
        );*/
    }
    pub fn process_connect(
        &mut self,
        msg: Message,
        sender_mapping: &HashMap<NodeIndex, Sender<Message>>,
    ) {
        if let Message::Connect(level, sender_index) = msg {
            if level < self.level {
                self.status.insert(sender_index, Status::Branch);
                let sender = sender_mapping
                    .get(&sender_index)
                    .expect("Error while reading 'sender_mapping':");
                let msg = Message::Initiate(self.level, self.name, self.state, self.index);
                sender
                    .send(msg.clone())
                    .expect("Error while sending message:");
            /*println!(
                "Thread [{:?}]: Sent message {:?} to {:?}",
                self.index, msg, sender_index
            );*/
            } else if *self
                .status
                .get(&sender_index)
                .expect("Error while reading 'status':")
                == Status::Basic
            {
                // wait
                /* Add the message to the end of the channel */
                let sender = sender_mapping
                    .get(&self.index)
                    .expect("Error while reading 'sender_mapping':");
                sender
                    .send(msg.clone())
                    .expect("Error while sending message:");
            /*println!(
                "Thread [{:?}]: Pushed message {:?} to the end of the channel",
                self.index, msg
            );*/
            } else {
                let sender = sender_mapping
                    .get(&sender_index)
                    .expect("Error while reading 'sender_mapping':");

                /* Finds the weight of the edge between 'self.index' and 'sender_index' */
                let mut wt = 0; // see if the initial value is correct
                {
                    let graph = self.graph.read().expect("Error while reading 'graph':");
                    for edge in graph.edges(self.index) {
                        let src = edge.source();
                        let target = edge.target();
                        let raw_edge = (self.index, sender_index);
                        if (src, target) == raw_edge || (target, src) == raw_edge {
                            // should get into at least once
                            wt = *edge.weight();
                            break;
                        }
                    }
                }
                let new_name = wt;

                let msg = Message::Initiate(self.level + 1, new_name, State::Find, self.index);
                sender
                    .send(msg.clone())
                    .expect("Error while sending message:");
                /*println!(
                    "Thread [{:?}]: Sent message {:?} to {:?}",
                    self.index, msg, sender_index
                );*/
            }
        } else {
            panic!("Wrong control flow!");
        }
    }
    pub fn process_initiate(
        &mut self,
        msg: Message,
        sender_mapping: &HashMap<NodeIndex, Sender<Message>>,
    ) {
        if let Message::Initiate(level, name, state, sender_index) = msg {
            self.level = level;
            self.name = name;
            self.state = state;
            self.parent = Some(sender_index);
            self.best_node = None;
            self.best_wt = std::i32::MAX;
            self.test_node = None;
            {
                let graph = self.graph.read().expect("Error while reading 'graph':");
                for nbr_index in graph.neighbors(self.index) {
                    if nbr_index != sender_index
                        && *self
                            .status
                            .get(&nbr_index)
                            .expect("Error while reading 'status':")
                            == Status::Branch
                    {
                        let sender = sender_mapping
                            .get(&nbr_index)
                            .expect("Error while reading 'sender_mapping':");
                        let msg = Message::Initiate(level, name, state, self.index);
                        sender
                            .send(msg.clone())
                            .expect("Error while sending message:");
                        /*println!(
                            "Thread [{:?}]: Sent message {:?} to {:?}",
                            self.index, msg, nbr_index
                        );*/
                    }
                }
            }
            if state == State::Find {
                self.rec = 0;
                self.find_min(sender_mapping);
            }
        } else {
            panic!("Wrong control flow!");
        }
    }
    pub fn find_min(&mut self, sender_mapping: &HashMap<NodeIndex, Sender<Message>>) {
        let mut min_edge = None;
        let mut q = None;
        let mut wt = std::i32::MAX;
        {
            let graph = self.graph.read().expect("Error while reading 'graph':");
            let edges = graph.edges(self.index);
            for edge in edges {
                let src = edge.source();
                let target = edge.target();
                let nbr_q = if src == self.index { target } else { src };
                if *self
                    .status
                    .get(&nbr_q)
                    .expect("Error while reading 'status':")
                    == Status::Basic
                    && (min_edge == None || *edge.weight() < wt)
                {
                    min_edge = Some(edge.id());
                    q = Some(nbr_q);
                    wt = *edge.weight();
                }
            }
        }
        if let Some(_edge) = min_edge {
            if let Some(nbr_q) = q {
                self.test_node = q;
                let sender = sender_mapping
                    .get(&nbr_q)
                    .expect("Error while reading 'sender_mapping':");
                let msg = Message::Test(self.level, self.name, self.index);
                sender
                    .send(msg.clone())
                    .expect("Error while sending message:"); // check clone()
                                                             /*println!(
                                                                 "Thread [{:?}]: Sent message {:?} to {:?}",
                                                                 self.index, msg, nbr_q
                                                             );*/
            } else {
                //println!("Invalid control flow!");
            }
        } else {
            self.test_node = None;
            self.report(sender_mapping);
        }
    }
    pub fn report(&mut self, sender_mapping: &HashMap<NodeIndex, Sender<Message>>) {
        let graph = self.graph.read().expect("Error while reading 'graph':");
        let mut cnt = 0;
        for q in graph.node_indices() {
            if *self.status.get(&q).expect("Error while reading 'status':") == Status::Branch
                && q != self.parent.expect("Error: parent found 'None':")
            {
                cnt += 1;
            }
        }
        if self.rec == cnt && self.test_node == None {
            self.state = State::Found;
            let sender = sender_mapping
                .get(&self.parent.expect("Error: parent found 'None':"))
                .expect("Error while reading 'sender_mapping':");
            let msg = Message::Report(self.best_wt, self.index);
            sender
                .send(msg.clone())
                .expect("Error while sending message:");
        /*println!(
            "Thread [{:?}]: Sent message {:?} to {:?}",
            self.index,
            msg,
            self.parent.unwrap()
        );*/
        } else {
            //skip
        }
    }
    pub fn process_test(
        &mut self,
        msg: Message,
        sender_mapping: &HashMap<NodeIndex, Sender<Message>>,
    ) {
        if let Message::Test(level, name, sender_index) = msg {
            if level > self.level {
                /* wait */
                /* Add the message to the end of the channel */
                let sender = sender_mapping
                    .get(&self.index)
                    .expect("Error while reading 'sender_mapping':");
                sender
                    .send(msg.clone())
                    .expect("Error while sending message:");
            /*println!(
                "Thread [{:?}]: Pushed message {:?} to the end of the channel",
                self.index, msg
            );*/
            } else if self.name == name {
                if *self
                    .status
                    .get(&sender_index)
                    .expect("Error while reading 'status':")
                    == Status::Basic
                {
                    self.status.insert(sender_index, Status::Reject);
                }
                /* Doing additional check : if self.test_node is 'None'  */
                /* Modification of the original algorithm */
                if self.test_node == None
                    || sender_index != self.test_node.expect("Error: test_node found 'None':")
                {
                    let sender = sender_mapping
                        .get(&sender_index)
                        .expect("Error while reading 'sender_mapping':");
                    let msg = Message::Reject(self.index);
                    sender
                        .send(msg.clone())
                        .expect("Error while sending message:");
                /*println!(
                    "Thread [{:?}]: Sent message {:?} to {:?}",
                    self.index, msg, sender_index
                );*/
                } else {
                    self.find_min(sender_mapping);
                }
            } else {
                let sender = sender_mapping
                    .get(&sender_index)
                    .expect("Error while reading 'sender_mapping':");
                let msg = Message::Accept(self.index);
                sender
                    .send(msg.clone())
                    .expect("Error while sending message:");
                /*println!(
                    "Thread [{:?}]: Sent message {:?} to {:?}",
                    self.index, msg, sender_index
                );*/
            }
        } else {
            panic!("Wrong control flow!");
        }
    }
    pub fn process_accept(
        &mut self,
        msg: Message,
        sender_mapping: &HashMap<NodeIndex, Sender<Message>>,
    ) {
        if let Message::Accept(sender_index) = msg {
            self.test_node = None;
            /* Finds the weight of the edge between 'self.index' and 'sender_index' */
            let mut wt = 0; // see if the initial value is correct
            {
                let graph = self.graph.read().expect("Error while reading 'graph':");
                for edge in graph.edges(self.index) {
                    let src = edge.source();
                    let target = edge.target();
                    let raw_edge = (self.index, sender_index);
                    if (src, target) == raw_edge || (target, src) == raw_edge {
                        // should get into at least once
                        wt = *edge.weight();
                        break;
                    }
                }
            }
            if wt < self.best_wt {
                self.best_wt = wt;
                self.best_node = Some(sender_index);
            }
            self.report(sender_mapping);
        } else {
            panic!("Wrong control flow!");
        }
    }
    pub fn process_reject(
        &mut self,
        msg: Message,
        sender_mapping: &HashMap<NodeIndex, Sender<Message>>,
    ) {
        if let Message::Reject(sender_index) = msg {
            if *self
                .status
                .get(&sender_index)
                .expect("Error while reading 'status':")
                == Status::Basic
            {
                self.status.insert(sender_index, Status::Reject);
            }
            self.find_min(sender_mapping);
        } else {
            panic!("Wrong control flow!");
        }
    }
    pub fn process_report(
        &mut self,
        msg: Message,
        sender_mapping: &HashMap<NodeIndex, Sender<Message>>,
    ) {
        if let Message::Report(wt, sender_index) = msg {
            if sender_index != self.parent.expect("Error: parent found 'None':") {
                if wt < self.best_wt {
                    self.best_wt = wt;
                    self.best_node = Some(sender_index);
                }
                self.rec += 1;
                self.report(sender_mapping);
            } else if self.state == State::Find {
                /* wait */
                /* Add the message to the end of the channel */
                let sender = sender_mapping
                    .get(&self.index)
                    .expect("Error while reading 'sender_mapping':");
                sender
                    .send(msg.clone())
                    .expect("Error while sending message:");
            /*println!(
                "Thread [{:?}]: Pushed message {:?} to the end of the channel",
                self.index, msg
            );*/
            } else if wt > self.best_wt {
                self.change_root(sender_mapping);
            } else if wt == self.best_wt && wt == std::i32::MAX {
                /* stop */
                let mut stop = self.stop.write().unwrap();
                *stop.get_mut() = true;
            } else {
                //invalid
            }
        } else {
            panic!("Wrong control flow!");
        }
    }
    pub fn change_root(&mut self, sender_mapping: &HashMap<NodeIndex, Sender<Message>>) {
        if *self
            .status
            .get(&self.best_node.expect("Error: best_node found 'None':"))
            .expect("Error while reading 'status':")
            == Status::Branch
        {
            let sender = sender_mapping
                .get(&self.best_node.expect("Error: best_node found 'None':"))
                .expect("Error while reading 'sender_mapping':");
            let msg = Message::ChangeRoot(self.index);
            sender
                .send(msg.clone())
                .expect("Error while sending message:");
        /*println!(
            "Thread [{:?}]: Sent message {:?} to {:?}",
            self.index,
            msg,
            self.best_node.unwrap()
        );*/
        } else {
            /* Whether to insert the status 'before' or 'after' the message is sent? */
            self.status.insert(
                self.best_node.expect("Error: best_node found 'None':"),
                Status::Branch,
            );
            let sender = sender_mapping
                .get(&self.best_node.expect("Error: best_node found 'None':"))
                .expect("Error while reading 'sender_mapping':");
            let msg = Message::Connect(self.level, self.index);
            sender
                .send(msg.clone())
                .expect("Error while sending message:");
            /*println!(
                "Thread [{:?}]: Sent message {:?} to {:?}",
                self.index,
                msg,
                self.best_node.unwrap()
            );*/
        }
    }
    pub fn process_change_root(
        &mut self,
        msg: Message,
        sender_mapping: &HashMap<NodeIndex, Sender<Message>>,
    ) {
        if let Message::ChangeRoot(_sender_index) = msg {
            self.change_root(sender_mapping);
        } else {
            panic!("Wrong control flow!");
        }
    }
}
