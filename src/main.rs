use petgraph::graph::{Graph, NodeIndex};
use petgraph::visit::EdgeRef;
use petgraph::Undirected;
use std::clone::Clone;
use std::collections::HashMap;
use std::str::FromStr;
use std::sync::atomic::AtomicBool;
use std::sync::mpsc::TryRecvError;
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::{Arc, RwLock};
use std::thread;
use std::{env, process};

mod node;
use node::*;

fn read_graph() -> Graph<i32, i32, Undirected> {
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
    let _nodes = u32::from_str(nodes).unwrap();
    //println!("No. of Nodes: {:?}", nodes);
    let mut graph: Graph<i32, i32, Undirected> = Graph::default();
    let mut edges_vec = vec![];
    let mut _edges = 0;
    for line in lines {
        _edges += 1;
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
    graph
}

fn get_mst_from_data(
    data: HashMap<NodeIndex, HashMap<NodeIndex, Status>>,
    graph: Arc<RwLock<Graph<i32, i32, Undirected>>>,
) -> Vec<(NodeIndex, NodeIndex, i32)> {
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
        let weight = *edge.weight();
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
        let triplet = (one, two, *weight);
        triplets.push(triplet);
    }
    //println!("Triplets: {:?}", triplets);
    triplets.sort_unstable_by(|(_, _, weight1), (_, _, weight2)| weight1.cmp(weight2));
    //println!("Sorted Triplets: {:?}", triplets);
    triplets
}

fn main() {
    let graph: Arc<RwLock<Graph<i32, i32, Undirected>>> = Arc::new(RwLock::new(read_graph()));
    let orig_mapping: Arc<RwLock<HashMap<NodeIndex, RwLock<Node>>>> =
        Arc::new(RwLock::new(HashMap::new()));
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
                        Message::Connect(_, _sender_index) => {
                            /*println!(
                                "Thread [{:?}]: Got message: {:?} from {:?}",
                                node_index, msg, sender_index
                            );*/
                            node.process_connect(msg, &sender_mapping);
                        }
                        Message::Initiate(_, _, _, _sender_index) => {
                            /*println!(
                                "Thread [{:?}]: Got message: {:?} from {:?}",
                                node_index, msg, sender_index
                            );*/
                            node.process_initiate(msg, &sender_mapping);
                        }
                        Message::Test(_, _, _sender_index) => {
                            /*println!(
                                "Thread [{:?}]: Got message: {:?} from {:?}",
                                node_index, msg, sender_index
                            );*/
                            node.process_test(msg, &sender_mapping);
                        }
                        Message::Accept(_sender_index) => {
                            /*println!(
                                "Thread [{:?}]: Got message: {:?} from {:?}",
                                node_index, msg, sender_index
                            );*/
                            node.process_accept(msg, &sender_mapping);
                        }
                        Message::Reject(_sender_index) => {
                            /*println!(
                                "Thread [{:?}]: Got message: {:?} from {:?}",
                                node_index, msg, sender_index
                            );*/
                            node.process_reject(msg, &sender_mapping);
                        }
                        Message::Report(_, _sender_index) => {
                            /*println!(
                                "Thread [{:?}]: Got message: {:?} from {:?}",
                                node_index, msg, sender_index
                            );*/
                            node.process_report(msg, &sender_mapping);
                            //println!("node.best_wt: {:?}", node.best_wt);
                        }
                        Message::ChangeRoot(_sender_index) => {
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

    let mst = get_mst_from_data(data, graph);

    for triplet in mst {
        let (one, two, three) = triplet;
        println!("({}, {}, {})", one.index(), two.index(), three);
    }
}
