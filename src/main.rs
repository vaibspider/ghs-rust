use petgraph::graph::{Graph, UnGraph};
use std::io::{self, BufRead};
use std::{env, process};
use std::fs::File;
use std::str::FromStr;
use petgraph::Undirected;
use petgraph::graph::NodeIndex;

fn main() {
  let mut args = env::args();
  if args.len() != 2 {
    println!("Usage: {} <input-file>", args.next().unwrap());
    process::exit(1);
  }
  let input_file = args.nth(1).unwrap();
  /*let mut input_file = File::open(input_file);
  let mut input_buffer = String::new();
  input_file.read_to_string(&mut input_buffer)?;*/

  let input_buffer = std::fs::read_to_string(input_file).unwrap();
  let input_buffer = input_buffer.as_str();
  let mut lines = input_buffer.lines();
  let mut lines_ref = &mut lines;
  let nodes = lines_ref.next().unwrap();
  let nodes = u32::from_str(nodes).unwrap();
  println!("No. of Nodes: {:?}", nodes);
  //let edges = lines_ref.count();
  let mut graph: Graph<i32, i32, Undirected> = Graph::default();
  /*for i in 0..nodes {
    graph.add_node(i);
  }*/
  println!("{:?}", graph);
  let mut tmp = vec![(5, 2, 3), (2, 5, 3), (6, 5, 7), (5, 6, 7)];
  /*graph.extend_with_edges(&tmp[..]);
  println!("{:?}", graph);*/
  let mut edges_vec = vec![];
  let mut edges = 0;
  for line in lines {
    edges += 1;
    //println!("{}", line);
    let tuple_vec: Vec<&str> = line.trim_matches(|p| p == '(' || p == ')')
        .split(',')
        .collect();
    //println!("{:?}", tuple_vec);
    let tuple_vec: Vec<i32> = tuple_vec.iter()
                             .map(|s| i32::from_str(s.trim()).unwrap())
                             .collect::<Vec<i32>>();
    //println!("{:?}", tuple_vec);
    let tuple = (tuple_vec[0] as u32, tuple_vec[1] as u32, tuple_vec[2]);
    println!("{:?}", tuple);
    edges_vec.push(tuple);
    /*let first = graph.add_node(tuple.0);
    let second = graph.add_node(tuple.1);
    graph.add_edge(first, second, tuple.2);*/
    //graph.add_edge(NodeIndex(tuple_vec[0]), NodeIndex(tuple_vec[1]), NodeIndex(tuple_vec[2]));
  }
  /*let a = (1, 2, 3);
  tmp.push(a);
  tmp.push((2, 1, 3));
  let a = (1, 2, 3);
  edges_vec.push(a);
  let a = (2, 1, 3);
  edges_vec.push(a);*/
  /*edges_vec.push(("1", "2", "3"));
  edges_vec.push(("2", "1", "3"));*/
  println!("{:?}", edges_vec);
  println!("No. of Edges: {}", edges);
  graph.extend_with_edges(&edges_vec[..]);
  println!("{:?}", graph);
  //println!("{}", input_buffer);

  /*let stdin = io::stdin();
  for line in stdin.lock().lines() {
    println!("{}", line.unwrap());
  }*/
}
