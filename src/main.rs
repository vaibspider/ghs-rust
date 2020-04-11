use petgraph::graph::{Graph, NodeIndex};
use petgraph::visit::EdgeRef;
use petgraph::Undirected;
use std::collections::HashMap;
use std::str::FromStr;
use std::sync::mpsc::{self, Receiver, Sender};
use std::{env, process};
use std::sync::{Arc, RwLock};
use std::thread;

enum State {
    Sleep,
    Find,
    Found,
}
enum Message {
    Connect(u32),                 // level
    Initiate(u32, String, State), // level, name, state
    Test(u32, String),            // level, name
    Accept,
    Reject,
    Report(u32), // bestWt
    ChangeRoot,
}
enum Status {
    Basic,
    Branch,
    Reject,
}
enum NodeId {
    Id(u32),
}
struct Node<'a> {
    index: NodeIndex,
    state: State,
    status: HashMap<NodeIndex, Status>,
    name: String,
    level: u32,
    parent: Option<NodeId>,
    best_wt: i32,
    best_node: Option<NodeId>,
    rec: u32,
    test_node: Option<NodeId>,
    graph: Arc<RwLock<Graph<i32, i32, Undirected>>>,
    receiver: Receiver<Message>,
    sender: Sender<Message>,
    mapping: Option<Arc<RwLock<HashMap<NodeIndex, RwLock<Node<'a>>>>>>,
}

impl<'a> Node<'a> {
    fn new(
        graph: Arc<RwLock<Graph<i32, i32, Undirected>>>,
        index: NodeIndex,
        mapping: Option<Arc<RwLock<HashMap<NodeIndex, RwLock<Node<'a>>>>>>,
    ) -> Self {
        // assuming that the calling function will add a mapping from this NodeIndex to Node
        let (sender, receiver) = mpsc::channel();
        let node = Node {
            index: index,
            state: State::Sleep,
            status: HashMap::new(),
            name: index.index().to_string(),
            level: 0,
            parent: None,
            best_wt: 0,
            best_node: None,
            rec: 0,
            test_node: None,
            graph: graph,
            sender: sender,
            receiver: receiver,
            mapping: mapping,
        };

        node
    }
    fn initialize(&mut self/*, mapping: Option<Arc<RwLock<HashMap<NodeIndex, RwLock<Node<'a>>>>>>*/) {
        println!("Entered initialize..");
        let graph = self.graph.read().unwrap();
        let edges = graph.edges(self.index);
        let edge_min = edges
            .min_by_key(|edge_ref| edge_ref.weight())
            .expect("Error while finding least weight edge during initialization");
        let src = edge_min.source();
        let target = edge_min.target();
        let nbr_q;
        if src == self.index {
            nbr_q = target;
        } else {
            nbr_q = src;
        }
        self.status.insert(nbr_q, Status::Branch);
        self.level = 0;
        self.state = State::Found;
        self.rec = 0;
        //self.mapping = mapping;
        println!("Set fields..");
        /*let first_mapping = self.mapping.as_ref().unwrap();
        println!("first_mapping");
        let tmp_mapping = first_mapping.lock().unwrap();
        println!("tmp_mapping");
        let node_q = tmp_mapping
            .get(&nbr_q)
            .expect("Error while getting node_q from nbr_q nodeindex");
        let node_q = node_q.lock().unwrap();
        println!("Got node_q lock..");
        let sender = node_q.sender.clone();
        println!("Cloned sender..");
        println!("sending..");
        sender.send(Message::Connect(0));
        println!("sent message..");*/
    }
    /*fn set_mapping(&mut self, mapping: Option<Arc<RwLock<HashMap<NodeIndex, RwLock<Node<'a>>>>>>) {
      self.mapping = mapping;
    }*/
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
    //let graph: Arc<RwLock<Graph<i32, i32, Undirected>>> = Arc::new(RwLock::new(Graph::default()));
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
    let copy_graph = graph.clone();
    let orig_mapping: HashMap<NodeIndex, RwLock<Node>> = HashMap::new();
    let orig_mapping: Arc<RwLock<HashMap<NodeIndex, RwLock<Node>>>> = Arc::new(RwLock::new(orig_mapping));
    for node_index in graph.read().unwrap().node_indices() {
      println!("creating node and mapping..");
      let node = Node::new(Arc::clone(&graph), node_index, None);
      let mut mapping = orig_mapping.write().unwrap();
      mapping.insert(node_index, RwLock::new(node));
      println!("created node and mapping..");
    }
    let mut handles = vec![];
    for node_index in graph.read().unwrap().node_indices() {
      let handle = thread::spawn(move || {
        println!("Thread no. {:?}", node_index);
        let mapping = orig_mapping.read().unwrap();
        let mut node = mapping.get(&node_index).unwrap();
        let mut node = node.write().unwrap();
      });
      handles.push(handle);
    }
    for handle in handles {
      handle.join();
    }
    println!("All threads finished!");
}
