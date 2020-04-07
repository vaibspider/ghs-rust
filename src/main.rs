use petgraph::graph::Graph;
use std::{env, process};
use std::str::FromStr;
use petgraph::Undirected;

fn main() {
  let mut args = env::args();
  if args.len() != 2 {
    println!("Usage: {} <input-file>", args.next().unwrap());
    process::exit(1);
  }
  let input_file = args.nth(1).unwrap();

  let input_buffer = std::fs::read_to_string(input_file).unwrap();
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
    let tuple_vec: Vec<&str> = line.trim_matches(|p| p == '(' || p == ')')
        .split(',')
        .collect();
    let tuple_vec: Vec<i32> = tuple_vec.iter()
                             .map(|s| i32::from_str(s.trim()).unwrap())
                             .collect::<Vec<i32>>();
    let tuple = (tuple_vec[0] as u32, tuple_vec[1] as u32, tuple_vec[2]);
    edges_vec.push(tuple);
  }
  println!("No. of Edges: {}", edges);
  graph.extend_with_edges(&edges_vec[..]);
  println!("{:?}", graph);
}
