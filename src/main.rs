use petgraph::algo::min_spanning_tree;
use petgraph::data::Element;
use petgraph::graph::{Graph, NodeIndex};
use petgraph::visit::EdgeRef;
use petgraph::Undirected;
use std::clone::Clone;
use std::cmp::PartialEq;
use std::collections::HashMap;
use std::fs::File;
use std::io::Write;
use std::marker::Copy;
use std::str::FromStr;
use std::sync::atomic::AtomicBool;
use std::sync::mpsc::TryRecvError;
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::{Arc, RwLock};
use std::thread;
use std::{env, process};

#[derive(PartialEq, Copy, Clone, Debug)]
enum State {
    Sleep,
    Find,
    Found,
}
#[derive(PartialEq, Clone, Debug)]
enum Message {
    Connect(u32, NodeIndex),              /* level */
    Initiate(u32, i32, State, NodeIndex), /* level, name, state */
    Test(u32, i32, NodeIndex),            /* level, name */
    Accept(NodeIndex),
    Reject(NodeIndex),
    Report(i32, NodeIndex), /* best_wt */
    ChangeRoot(NodeIndex),
}
#[derive(PartialEq, Copy, Clone)]
enum Status {
    Basic,
    Branch,
    Reject,
}
struct Node {
    index: NodeIndex,
    state: State,
    status: HashMap<NodeIndex, Status>,
    name: i32,
    level: u32,
    parent: Option<NodeIndex>,
    best_wt: i32,
    best_node: Option<NodeIndex>,
    rec: u32,
    test_node: Option<NodeIndex>,
    graph: Arc<RwLock<Graph<i32, i32, Undirected>>>,
    stop: Arc<RwLock<AtomicBool>>,
}

impl Node {
    fn new(
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
    fn initialize(&mut self, sender_mapping: &HashMap<NodeIndex, Sender<Message>>) {
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
    fn process_connect(
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
    fn process_initiate(
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
                        let msg = Message::Initiate(
                            level,
                            name,
                            state, self.index,
                        );
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
    fn find_min(&mut self, sender_mapping: &HashMap<NodeIndex, Sender<Message>>) {
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
    fn report(&mut self, sender_mapping: &HashMap<NodeIndex, Sender<Message>>) {
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
    fn process_test(&mut self, msg: Message, sender_mapping: &HashMap<NodeIndex, Sender<Message>>) {
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
    fn process_accept(
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
    fn process_reject(
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
    fn process_report(
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
    fn change_root(&mut self, sender_mapping: &HashMap<NodeIndex, Sender<Message>>) {
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
    fn process_change_root(
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
    //println!("No. of Nodes: {:?}", nodes);
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
    //println!("No. of Edges: {}", edges);
    graph.extend_with_edges(&edges_vec[..]);
    //println!("{:?}", graph);

    /*
    let mut kruskal_output_file = File::create("kruskal_output.mst").unwrap();
    let kruskal_mst = min_spanning_tree(&graph);
    for item in kruskal_mst {
        if let Element::Edge {
            source,
            target,
            weight,
        } = item
        {
            writeln!(kruskal_output_file, "({}, {}, {})", source, target, weight).unwrap();
        }
    }
    */

    let graph: Arc<RwLock<Graph<i32, i32, Undirected>>> = Arc::new(RwLock::new(graph));
    let orig_mapping: HashMap<NodeIndex, RwLock<Node>> = HashMap::new();
    let orig_mapping: Arc<RwLock<HashMap<NodeIndex, RwLock<Node>>>> =
        Arc::new(RwLock::new(orig_mapping));
    let stop = Arc::new(RwLock::new(AtomicBool::new(false)));
    for node_index in graph
        .read()
        .expect("Error while reading 'graph':")
        .node_indices()
    {
        //println!("creating node and mapping..");
        let node = Node::new(Arc::clone(&graph), node_index, Arc::clone(&stop));
        let mut mapping = orig_mapping.write().unwrap();
        mapping.insert(node_index, RwLock::new(node));
        //println!("created node and mapping..");
    }

    let mut sender_mapping: HashMap<NodeIndex, Sender<Message>> = HashMap::new();
    let mut receiver_mapping: HashMap<NodeIndex, Receiver<Message>> = HashMap::new();
    for node_index in graph
        .read()
        .expect("Error while reading 'graph':")
        .node_indices()
    {
        let (sender, receiver) = mpsc::channel();
        sender_mapping.insert(node_index, sender);
        receiver_mapping.insert(node_index, receiver);
    }

    let mut handles = vec![];
    for node_index in graph
        .read()
        .expect("Error while reading 'graph':")
        .node_indices()
    {
        let move_mapping = Arc::clone(&orig_mapping);
        let sender_mapping = sender_mapping.clone();
        let receiver = receiver_mapping.remove(&node_index).unwrap();
        let handle = thread::Builder::new()
            .name(node_index.index().to_string())
            .spawn(move || {
                //println!("Thread no. {:?} started!", node_index);
                let receiver = receiver;
                let sender_mapping = sender_mapping;
                let mapping = move_mapping.read().unwrap();
                let node = mapping.get(&node_index).unwrap();
                /* is it okay to keep 'node' mutated throught this thread's scope? */
                let mut node = node.write().unwrap();
                /* Should we wakeup (initialize) all the nodes? */
                node.initialize(&sender_mapping);
                loop {
                    {
                        if *node.stop.write().unwrap().get_mut() {
                            break;
                        }
                    }
                    /* Blocking receive? OR Non-blocking? */
                    let recv = receiver.try_recv();
                    let msg = match recv {
                        Err(TryRecvError::Empty) => {
                            continue;
                        }
                        Err(TryRecvError::Disconnected) => {
                            break;
                        }
                        Ok(message) => message,
                    };
                    match msg {
                        Message::Connect(_, sender_index) => {
                            /*println!(
                                "Thread [{:?}]: Got message: {:?} from {:?}",
                                node_index, msg, sender_index
                            );*/
                            node.process_connect(msg, &sender_mapping);
                        }
                        Message::Initiate(_, _, _, sender_index) => {
                            /*println!(
                                "Thread [{:?}]: Got message: {:?} from {:?}",
                                node_index, msg, sender_index
                            );*/
                            node.process_initiate(msg, &sender_mapping);
                        }
                        Message::Test(_, _, sender_index) => {
                            /*println!(
                                "Thread [{:?}]: Got message: {:?} from {:?}",
                                node_index, msg, sender_index
                            );*/
                            node.process_test(msg, &sender_mapping);
                        }
                        Message::Accept(sender_index) => {
                            /*println!(
                                "Thread [{:?}]: Got message: {:?} from {:?}",
                                node_index, msg, sender_index
                            );*/
                            node.process_accept(msg, &sender_mapping);
                        }
                        Message::Reject(sender_index) => {
                            /*println!(
                                "Thread [{:?}]: Got message: {:?} from {:?}",
                                node_index, msg, sender_index
                            );*/
                            node.process_reject(msg, &sender_mapping);
                        }
                        Message::Report(_, sender_index) => {
                            /*println!(
                                "Thread [{:?}]: Got message: {:?} from {:?}",
                                node_index, msg, sender_index
                            );*/
                            node.process_report(msg, &sender_mapping);
                            //println!("node.best_wt: {:?}", node.best_wt);
                        }
                        Message::ChangeRoot(sender_index) => {
                            /*println!(
                                "Thread [{:?}]: Got message: {:?} from {:?}",
                                node_index, msg, sender_index
                            );*/
                            node.process_change_root(msg, &sender_mapping);
                        }
                    }
                }
                //println!("Thread no. {:?} Stopped!", node_index);
                (node_index, node.status.clone())
            });
        handles.push(handle);
    }
    //println!("All threads finished!");
    let mut data: HashMap<NodeIndex, HashMap<NodeIndex, Status>> = HashMap::new();
    for handle in handles {
        let (node_index, status_map) = handle
            .expect("Error while unwrapping 'handle':")
            .join()
            .expect("Error while unwrapping 'handle.join()':");
        data.insert(node_index, status_map);
    }
    let mut pairs = vec![];
    for (node_index, status_map) in data {
        for (nbr_index, status) in status_map {
            if status == Status::Branch {
                let pair = if node_index < nbr_index {
                    (node_index, nbr_index)
                } else {
                    (nbr_index, node_index)
                };
                if !pairs.contains(&pair) {
                    pairs.push(pair);
                }
            }
        }
    }
    //println!("Pairs: {:?}", pairs);

    let mut weight_map = HashMap::new();
    let graph = graph.read().unwrap();
    for edge in graph.edge_references() {
        let source = edge.source();
        let target = edge.target();
        let weight = edge.weight();
        weight_map.insert((source, target), weight);
    }

    let mut triplets = vec![];
    for pair in pairs {
        let (one, two) = pair;
        let mut new_pair = (one, two);
        let weight = if weight_map.get(&new_pair) == None {
            new_pair = (two, one);
            weight_map
                .get(&new_pair)
                .expect("Error while getting a pair weight from weight_map:")
        } else {
            weight_map
                .get(&new_pair)
                .expect("Error while getting a pair weight from weight_map:")
        };
        let (one, two) = new_pair;
        let triplet = (one, two, weight);
        triplets.push(triplet);
    }
    //println!("Triplets: {:?}", triplets);
    triplets.sort_unstable_by(|(_, _, weight1), (_, _, weight2)| weight1.cmp(weight2));
    //println!("Sorted Triplets: {:?}", triplets);

    /*let mut output_file = File::create("ghs_output.mst").unwrap();
    for triplet in triplets {
        let (one, two, three) = triplet;
        writeln!(output_file, "({}, {}, {})", one.index(), two.index(), three).unwrap();
    }*/
    for triplet in triplets {
        let (one, two, three) = triplet;
        println!("({}, {}, {})", one.index(), two.index(), three);
    }
}
