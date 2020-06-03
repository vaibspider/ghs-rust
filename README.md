# GHS Algorithm implementation in Rust

### Build command:
`cargo build` (for debug build) or 
`cargo build --release`

### Run GHS:
`./target/debug/ghs <input-file>` (if using debug build) or
`./target/release/ghs <input-file>`

Input for GHS is a connected, undirected graph with unique edge weights.

### Format for 'input-file':
See example file - [input](https://github.com/vaibspider/ghs-rust/blob/module/input).

The first line is the number of nodes in the input graph.  
Every line following the first line is an edge in the format - `(nodeindex, nodeindex, weight)`.
