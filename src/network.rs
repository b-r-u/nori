use std::collections::HashMap;
use std::fs::File;
use std::io::{BufWriter, Write};
use std::path::Path;

use osrmreader::{Entry, OsrmReader};


pub struct Network {
    nodes_vec: Vec<(i64, i32, i32)>,
    edges_map: HashMap<(i64, i64), (u32, u32, usize)>,
}

impl Network {
    pub fn bump_edges(&mut self, nodes: &[i64]) {
        for win in nodes.windows(2) {
            if win.len() == 2 {
                match self.edges_map.get_mut(&(win[0], win[1])) {
                    Some(val) => val.2 += 1,
                    None => {},//println!("lookup fail ({}, {})", win[0], win[1]),
                }
            }
        }
    }

    pub fn write_to_geojson<P: AsRef<Path>>(&self, output_path: P) -> Result<(), Box<dyn std::error::Error>> {
        let mut output = BufWriter::new(File::create(output_path)?);
        output.write(b"{\"type\": \"FeatureCollection\", \"features\": [")?;

        let mut first = true;
        for (source_index, target_index, number) in self.edges_map.values() {
            // Look up nodes
            let source = self.nodes_vec[*source_index as usize];
            let target = self.nodes_vec[*target_index as usize];

            if *number < 1 {
                continue;
            }

            if !first {
                write!(output, ",")?;
            }
            first = false;

            // Convert raw coordinates and keep full precision for writing the line string
            write!(
                output,
                "\n{{\"type\": \"Feature\", \"properties\": {{\"number\": {}}}, \"geometry\": {{\"type\": \"LineString\", \"coordinates\": [[{:.6}, {:.6}], [{:.6}, {:.6}]]}}}}",
                number,
                source.2 as f64 * 0.000001,
                source.1 as f64 * 0.000001,
                target.2 as f64 * 0.000001,
                target.1 as f64 * 0.000001,
            )?;
        }

        // Properly end the GeoJSON file
        output.write(b"\n]}")?;
        output.flush()?;

        Ok(())
    }


    pub fn from_path<P: AsRef<Path>>(path: P) -> Result<Network, Box<dyn std::error::Error>> {
        let f = std::fs::File::open(path)?;
        let mut reader = OsrmReader::new(f);
        let mut nodes_vec = vec![];
        let mut edges_map = HashMap::new();

        for entry in reader.entries()? {
            match entry? {
                Entry::Nodes(nodes) => {
                    // Read nodes
                    for n in nodes {
                        let n = n?;
                        nodes_vec.push((n.node_id, n.raw_latitude, n.raw_longitude))
                    }
                },
                Entry::Edges(edges) => {
                    // Read edges
                    for e in edges {
                        let e = e?;
                        let a = nodes_vec[e.source_node_index as usize];
                        let b = nodes_vec[e.target_node_index as usize];
                        edges_map.insert((a.0, b.0), (e.source_node_index, e.target_node_index, 0));
                    }
                },
                _ => {},
            }
        }

        println!("number edges {}", edges_map.len());

        Ok(Network {
            nodes_vec,
            edges_map,
        })
    }
}
