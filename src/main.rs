use petgraph::graph::{self, Graph, NodeIndex};
use petgraph::Undirected;
use std::str::FromStr;
use std::{env, process};
use petgraph::visit::EdgeRef;
use std::collections::HashMap;
use std::sync::mpsc::{self, Sender, Receiver};

enum State {
    Sleep,
    Find,
    Found,
}
enum Message {
  Connect(u32), // level
  Initiate(u32, String, State), // level, name, state
  Test(u32, String), // level, name
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
struct Node {
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
    graph_node: graph::Node<i32>,
    graph: Graph<i32, i32, Undirected>,
    receiver: Receiver<Message>,
    sender: Sender<Message>,
}

impl Node {
    fn new(&self, graph_node: graph::Node<i32>, graph: Graph<i32, i32, Undirected>, index: NodeIndex) -> Self {
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
            graph_node: graph_node,
            graph: graph,
            sender: sender,
            receiver: receiver,
        };
        node
    }
    fn initialize(&mut self) {
        let mut edges = self.graph.edges(self.index);
        let edge_min = edges.min_by_key(|edge_ref| edge_ref.weight()).expect("Error while finding least weight edge during initialization");
        let src = edge_min.source();
        let target = edge_min.target();
        let nbr_q;
        if src == self.index {
          nbr_q = target;
        }
        else {
          nbr_q = src;
        }
        self.status.insert(nbr_q, Status::Branch);
        self.level = 0;
        self.state = State::Found;
        self.rec = 0;
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
}
