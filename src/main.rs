use petgraph::graph::{Graph, NodeIndex};
use petgraph::visit::EdgeRef;
use petgraph::Undirected;
use std::clone::Clone;
use std::cmp::PartialEq;
use std::collections::HashMap;
use std::marker::Copy;
use std::str::FromStr;
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::{Arc, RwLock};
use std::thread;
use std::{env, process};

#[derive(PartialEq, Copy, Clone)]
enum State {
    Sleep,
    Find,
    Found,
}
#[derive(PartialEq)]
enum Message {
    Connect(u32, NodeIndex),                 // level
    Initiate(u32, String, State, NodeIndex), // level, name, state
    Test(u32, String, NodeIndex),            // level, name
    Accept(NodeIndex),
    Reject(NodeIndex),
    Report(i32, NodeIndex), // bestWt
    ChangeRoot(NodeIndex),
}
#[derive(PartialEq)]
enum Status {
    Basic,
    Branch,
    Reject,
}
struct Node {
    index: NodeIndex,
    state: State,
    status: HashMap<NodeIndex, Status>,
    name: String,
    level: u32,
    parent: Option<NodeIndex>,
    best_wt: i32,
    best_node: Option<NodeIndex>,
    rec: u32,
    test_node: Option<NodeIndex>,
    graph: Arc<RwLock<Graph<i32, i32, Undirected>>>,
}

impl Node {
    fn new(graph: Arc<RwLock<Graph<i32, i32, Undirected>>>, index: NodeIndex) -> Self {
        Node {
            index,
            state: State::Sleep,
            status: HashMap::new(),
            name: index.index().to_string(),
            level: 0,
            parent: None,
            best_wt: std::i32::MAX,
            best_node: None,
            rec: 0,
            test_node: None,
            graph,
        }
    }
    fn initialize(
        &mut self,
        sender_mapping: &HashMap<NodeIndex, Sender<Message>>,
        _receiver: &Receiver<Message>,
    ) {
        println!("Entered initialize..");
        let graph = self.graph.read().unwrap();
        let edges = graph.edges(self.index);
        let edge_min = edges
            .min_by_key(|edge_ref| edge_ref.weight())
            .expect("Error while finding least weight edge during initialization");
        let src = edge_min.source();
        let target = edge_min.target();
        let nbr_q = if src == self.index { target } else { src };
        self.status.insert(nbr_q, Status::Branch);
        for node_index in graph.node_indices() {
            if node_index != nbr_q {
                self.status.insert(node_index, Status::Basic);
            }
        }
        self.level = 0;
        self.state = State::Found;
        self.rec = 0;
        let sender = sender_mapping.get(&nbr_q).unwrap();
        sender.send(Message::Connect(0, self.index)).unwrap();
        println!("Set fields..");
        println!("sent message..");
    }
    fn process_connect(
        &mut self,
        msg: Message,
        sender_mapping: &HashMap<NodeIndex, Sender<Message>>,
    ) {
        if let Message::Connect(level, sender_index) = msg {
            if level < self.level {
                self.status.insert(sender_index, Status::Branch);
                let sender = sender_mapping.get(&sender_index).unwrap();
                sender
                    .send(Message::Initiate(
                        self.level,
                        self.name.clone(),
                        self.state,
                        self.index,
                    ))
                    .unwrap();
            } else if *self.status.get(&sender_index).unwrap() == Status::Basic {
                // wait (do nothing?)
            } else {
                let sender = sender_mapping.get(&sender_index).unwrap();
                sender
                    .send(Message::Initiate(
                        self.level + 1,
                        format!("{}{}", self.name, sender_index.index().to_string()),
                        State::Find,
                        self.index,
                    ))
                    .unwrap();
            }
        } else {
            panic!("Wrong message!");
        }
    }
    fn process_initiate(
        &mut self,
        msg: Message,
        sender_mapping: &HashMap<NodeIndex, Sender<Message>>,
    ) {
        if let Message::Initiate(level, name, state, sender_index) = msg {
            self.level = level;
            self.name = name.clone(); // could be a potential problem
            self.state = state;
            self.parent = Some(sender_index);
            self.best_node = None;
            self.best_wt = std::i32::MAX;
            self.test_node = None;
            {
                let graph = self.graph.read().unwrap();
                for nbr_index in graph.neighbors(self.index) {
                    if nbr_index != sender_index
                        && *self.status.get(&nbr_index).unwrap() == Status::Branch
                    {
                        let sender = sender_mapping.get(&nbr_index).unwrap();
                        sender
                            .send(Message::Initiate(
                                level,
                                name.clone(), // could be a potential problem
                                state,
                                self.index,
                            ))
                            .unwrap();
                    }
                }
            }
            if state == State::Find {
                self.rec = 0;
                self.find_min(sender_mapping);
            }
        } else {
            panic!("Wrong message!");
        }
    }
    fn find_min(&mut self, sender_mapping: &HashMap<NodeIndex, Sender<Message>>) {
        let mut min_edge = None;
        let mut q = None;
        let mut wt = 0; // check initial value is correct or not
        {
            let graph = self.graph.read().unwrap();
            let edges = graph.edges(self.index);
            for edge in edges {
                let src = edge.source();
                let target = edge.target();
                let nbr_q = if src == self.index { target } else { src };
                if *self.status.get(&nbr_q).unwrap() == Status::Basic {
                    if min_edge == None {
                        min_edge = Some(edge.id());
                        q = Some(nbr_q);
                        wt = *edge.weight();
                    } else {
                        if *edge.weight() < wt {
                            min_edge = Some(edge.id());
                            q = Some(nbr_q);
                            wt = *edge.weight();
                        } else {
                            //skip
                        }
                    }
                }
            }
        }
        if let Some(edge) = min_edge {
            if let Some(nbr_q) = q {
                self.test_node = q;
                let sender = sender_mapping.get(&nbr_q).unwrap();
                sender
                    .send(Message::Test(self.level, self.name.clone(), self.index))
                    .unwrap(); // check clone()
            } else {
                //invalid / impossible
            }
        } else {
            self.test_node = None;
            self.report(sender_mapping);
        }
    }
    fn report(&mut self, sender_mapping: &HashMap<NodeIndex, Sender<Message>>) {
        let graph = self.graph.read().unwrap();
        let mut cnt = 0;
        for q in graph.node_indices() {
            if *self.status.get(&q).unwrap() == Status::Branch && q != self.parent.unwrap() {
                cnt += 1;
            }
        }
        if self.rec == cnt && self.test_node == None {
            self.state = State::Found;
            let sender = sender_mapping.get(&self.parent.unwrap()).unwrap();
            sender
                .send(Message::Report(self.best_wt, self.index))
                .unwrap();
        } else {
            //skip
        }
    }
    fn process_test(&mut self, msg: Message, sender_mapping: &HashMap<NodeIndex, Sender<Message>>) {
        if let Message::Test(level, name, sender_index) = msg {
            if level > self.level {
                //wait
            } else if self.name == name {
                if *self.status.get(&sender_index).unwrap() == Status::Basic {
                    self.status.insert(sender_index, Status::Reject);
                }
                if sender_index != self.test_node.unwrap() {
                    let sender = sender_mapping.get(&sender_index).unwrap();
                    sender.send(Message::Reject(self.index)).unwrap();
                } else {
                    self.find_min(sender_mapping);
                }
            } else {
                let sender = sender_mapping.get(&sender_index).unwrap();
                sender.send(Message::Accept(self.index)).unwrap();
            }
        } else {
            // invalid message
        }
    }
    fn process_accept(
        &mut self,
        msg: Message,
        sender_mapping: &HashMap<NodeIndex, Sender<Message>>,
    ) {
        if let Message::Accept(sender_index) = msg {
            self.test_node = None;
            let mut wt = 0; // see if the initial value is correct
            {
                let graph = self.graph.read().unwrap();
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
            //invalid
        }
    }
    fn process_reject(
        &mut self,
        msg: Message,
        sender_mapping: &HashMap<NodeIndex, Sender<Message>>,
    ) {
        if let Message::Reject(sender_index) = msg {
            if *self.status.get(&sender_index).unwrap() == Status::Basic {
                self.status.insert(sender_index, Status::Reject);
            }
            self.find_min(sender_mapping);
        } else {
            //invalid
        }
    }
    fn process_report(
        &mut self,
        msg: Message,
        sender_mapping: &HashMap<NodeIndex, Sender<Message>>,
    ) {
        if let Message::Report(wt, sender_index) = msg {
            if sender_index != self.parent.unwrap() {
                if wt < self.best_wt {
                    self.best_wt = wt;
                    self.best_node = Some(sender_index);
                }
                self.rec += 1;
                self.report(sender_mapping);
            } else {
                if self.state == State::Find {
                    //wait
                } else if wt > self.best_wt {
                    self.change_root();
                } else if wt == self.best_wt && wt == std::i32::MAX {
                    //stop (what to do here?)
                } else {
                    //invalid? do nothing?
                }
            }
        } else {
            //invalid
        }
    }
    fn change_root(&self) {}
}

fn main() {
    let mut args = env::args();
    if args.len() != 2 {
        println!("Usage: {} <input-file>", args.next().unwrap());
        process::exit(1);
    }
    let input_file = args.nth(1).unwrap();

    let input_buffer = std::fs::read_to_string(input_file).expect("Unable to open input file");
    let input_buffer = input_buffer.as_str();
    let mut lines = input_buffer.lines();
    let lines_ref = &mut lines;
    let nodes = lines_ref.next().unwrap();
    let nodes = u32::from_str(nodes).unwrap();
    println!("No. of Nodes: {:?}", nodes);
    let mut graph: Graph<i32, i32, Undirected> = Graph::default();
    let mut edges_vec = vec![];
    let mut edges = 0;
    for line in lines {
        edges += 1;
        let tuple_vec: Vec<&str> = line
            .trim_matches(|p| p == '(' || p == ')')
            .split(',')
            .collect();
        let tuple_vec: Vec<i32> = tuple_vec
            .iter()
            .map(|s| i32::from_str(s.trim()).unwrap())
            .collect::<Vec<i32>>();
        let tuple = (tuple_vec[0] as u32, tuple_vec[1] as u32, tuple_vec[2]);
        edges_vec.push(tuple);
    }
    println!("No. of Edges: {}", edges);
    graph.extend_with_edges(&edges_vec[..]);
    println!("{:?}", graph);

    let graph: Arc<RwLock<Graph<i32, i32, Undirected>>> = Arc::new(RwLock::new(graph));
    let orig_mapping: HashMap<NodeIndex, RwLock<Node>> = HashMap::new();
    let orig_mapping: Arc<RwLock<HashMap<NodeIndex, RwLock<Node>>>> =
        Arc::new(RwLock::new(orig_mapping));
    for node_index in graph.read().unwrap().node_indices() {
        println!("creating node and mapping..");
        let node = Node::new(Arc::clone(&graph), node_index);
        let mut mapping = orig_mapping.write().unwrap();
        mapping.insert(node_index, RwLock::new(node));
        println!("created node and mapping..");
    }

    let mut sender_mapping: HashMap<NodeIndex, Sender<Message>> = HashMap::new();
    let mut receiver_mapping: HashMap<NodeIndex, Receiver<Message>> = HashMap::new();
    for node_index in graph.read().unwrap().node_indices() {
        let (sender, receiver) = mpsc::channel();
        sender_mapping.insert(node_index, sender);
        receiver_mapping.insert(node_index, receiver);
    }

    let mut handles = vec![];
    for node_index in graph.read().unwrap().node_indices() {
        let move_mapping = Arc::clone(&orig_mapping);
        let sender_mapping = sender_mapping.clone();
        let receiver = receiver_mapping.remove(&node_index).unwrap();
        let handle = thread::Builder::new()
            .name(node_index.index().to_string())
            .spawn(move || {
                println!("Thread no. {:?}", node_index);
                let receiver = receiver;
                let sender_mapping = sender_mapping;
                let mapping = move_mapping.read().unwrap();
                let node = mapping.get(&node_index).unwrap();
                let mut node = node.write().unwrap();
                node.initialize(&sender_mapping, &receiver);
                for _recv in receiver {
                    println!("Thread [{:?}]: Got message", node_index);
                }
            });
        handles.push(handle);
    }
    for handle in handles {
        handle.unwrap().join().unwrap();
    }
    println!("All threads finished!");
}
