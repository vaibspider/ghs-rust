use petgraph::graph::{self, Graph};
use petgraph::Undirected;
use std::str::FromStr;
use std::{env, process};

enum State {
    Sleep,
    Find,
    Found,
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
    id: NodeId,
    state: State,
    status: Vec<Status>,
    name: String,
    level: u32,
    parent: Option<NodeId>,
    best_wt: i32,
    best_node: Option<NodeId>,
    rec: u32,
    test_node: Option<NodeId>,
    graph_node: graph::Node<i32>,
}

impl Node {
    fn new(&self, graph_node: graph::Node<i32>) -> Self {
        let wt: u32 = graph_node.weight as u32;
        let node = Node {
            id: NodeId::Id(wt),
            state: State::Sleep,
            status: Vec::new(),
            name: wt.to_string(),
            level: 0,
            parent: None,
            best_wt: 0,
            best_node: None,
            rec: 0,
            test_node: None,
            graph_node: graph_node,
        };
        node
    }
    fn initialize(&mut self) {
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
