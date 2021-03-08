use std::collections::HashMap;
use std::path::Path;

use geomatic::{laea, Point3035, Point4326};
use osrmreader::{Entry, OsrmReader};
use serde::{Serialize, Deserialize};

use crate::bounding_box::BoundingBox;
use crate::geojson_writer::GeoJsonWriter;
use crate::polyline::PolylineCollection;


#[derive(Copy, Clone, Debug, Eq, Hash, PartialEq, Serialize, Deserialize)]
pub struct OsmNodeId(i64);

#[derive(Copy, Clone, Debug, Eq, Hash, PartialEq, Serialize, Deserialize)]
pub struct NodeId(u32);

#[derive(Copy, Clone, Debug, Eq, Hash, PartialEq, Serialize, Deserialize)]
pub struct EdgeId(u32);

/// An undefined OSM edge.
/// TODO Maybe use NonZeroI64 for OsmNodeId?
pub const UNDEF_OSM_EDGE: (OsmNodeId, OsmNodeId) = (OsmNodeId(0), OsmNodeId(0));


#[derive(Copy, Clone, Debug, Eq, Hash, PartialEq)]
pub struct Node {
    osm_node_id: OsmNodeId,
    raw_lat: i32,
    raw_lon: i32,
}

#[derive(Copy, Clone, Debug, Eq, Hash, PartialEq)]
struct Edge {
    source_node_id: NodeId,
    target_node_id: NodeId,
    number: usize,
}

pub struct Network {
    nodes_vec: Vec<Node>,
    edges_vec: Vec<Edge>,
    edges_map: HashMap<(NodeId, NodeId), EdgeId>,
    osm_2_node_id: HashMap<OsmNodeId, NodeId>,
}

pub struct FullEdge {
    /// first point
    pub a: Node,
    /// second point
    pub b: Node,
    /// Number of routes that passed trough this edge
    pub number: usize,
}

impl Node {
    pub fn as_point4326(&self) -> Point4326 {
        Point4326::new(self.raw_lat as f64 * 0.000001, self.raw_lon as f64 * 0.000001)
    }

    pub fn as_point3035(&self) -> Point3035 {
        laea::forward(self.as_point4326())
    }
}

impl FullEdge {
    pub fn osm_ids(&self) -> (OsmNodeId, OsmNodeId) {
        (self.a.osm_node_id, self.b.osm_node_id)
    }
}

impl Network {
    pub fn get_node(&self, osm_node_id: OsmNodeId) -> Option<Node> {
        self.osm_2_node_id
            .get(&osm_node_id)
            .and_then(|id| self.nodes_vec.get(id.0 as usize).copied())
    }

    pub fn bump_edges(&mut self, nodes: &[OsmNodeId]) {
        for win in nodes.windows(2) {
            let a_id = self.osm_2_node_id.get(&win[0]);
            let b_id = self.osm_2_node_id.get(&win[1]);
            if let (Some(&a_id), Some(&b_id)) = (a_id, b_id) {
                // look for edge a -> b
                if let Some(edge_index) = self.edges_map.get_mut(&(a_id, b_id)) {
                    self.edges_vec[edge_index.0 as usize].number += 1;
                    continue;
                }
                // look for reversed edge b -> a
                match self.edges_map.get_mut(&(b_id, a_id)) {
                    Some(edge_index) => self.edges_vec[edge_index.0 as usize].number += 1,
                    None => println!("lookup fail ({:?}, {:?})", win[0], win[1]),
                }
            }
        }
    }

    pub fn write_to_geojson<P: AsRef<Path>>(&self, output_path: P) -> anyhow::Result<()> {
        let mut writer = GeoJsonWriter::from_path(output_path)?;

        for edge in self.edges() {
            if edge.number < 1 {
                continue;
            }

            let mut ls = writer.add_line_string(&[edge.a.as_point4326(), edge.b.as_point4326()])?;
            ls.add_property("number", edge.number)?;
            ls.finish()?;
        }

        writer.finish()?;

        Ok(())
    }

    pub fn edges(&self) -> impl Iterator<Item=FullEdge> + '_ {
        self.edges_vec.iter().map(move |edge| {
            let source = self.nodes_vec[edge.source_node_id.0 as usize];
            let target = self.nodes_vec[edge.target_node_id.0 as usize];
            FullEdge {
                a: source,
                b: target,
                number: edge.number,
            }
        })
    }

    pub fn from_path<P: AsRef<Path>>(path: P) -> Result<Network, std::io::Error> {
        let f = std::fs::File::open(path)?;
        let mut reader = OsrmReader::new(f);
        let mut nodes_vec = vec![];
        let mut edges_vec = vec![];
        let mut edges_map = HashMap::new();
        let mut osm_2_node_id = HashMap::new();

        for entry in reader.entries()? {
            match entry? {
                Entry::Nodes(nodes) => {
                    // Read nodes
                    for n in nodes {
                        let n = n?;
                        osm_2_node_id.insert(OsmNodeId(n.node_id), NodeId(nodes_vec.len() as u32));
                        nodes_vec.push(Node {
                            osm_node_id: OsmNodeId(n.node_id),
                            raw_lat: n.raw_latitude,
                            raw_lon: n.raw_longitude,
                        })
                    }
                },
                Entry::Edges(edges) => {
                    // Read edges
                    let mut edge_index = 0;
                    for e in edges {
                        let e = e?;
                        let source_id = NodeId(e.source_node_index);
                        let target_id = NodeId(e.target_node_index);
                        edges_map.insert((source_id, target_id), EdgeId(edge_index));
                        edges_vec.push(
                            Edge {
                                source_node_id: source_id,
                                target_node_id: target_id,
                                number: 0,
                            }
                        );
                        edge_index += 1;
                    }
                },
                _ => {},
            }
        }

        println!("number edges {}", edges_vec.len());

        Ok(Network {
            nodes_vec,
            edges_vec,
            edges_map,
            osm_2_node_id,
        })
    }

    pub fn get_bounds(&self) -> BoundingBox {
        let mut edges_iter = self.edges();

        let mut min_x = 0.0;
        let mut max_x = 0.0;
        let mut min_y = 0.0;
        let mut max_y = 0.0;

        if let Some(edge) = edges_iter.next() {
            let a = edge.a.as_point4326();
            let b = edge.b.as_point4326();
            min_x = a.lon().min(b.lon());
            min_y = a.lat().min(b.lat());
            max_x = a.lon().max(b.lon());
            max_y = a.lat().max(b.lat());
        }

        for edge in edges_iter {
            let a = edge.a.as_point4326();
            let b = edge.b.as_point4326();
            if a.lon() < min_x {
                min_x = a.lon();
            }
            if b.lon() < min_x {
                min_x = b.lon();
            }
            if a.lat() < min_y {
                min_y = a.lat();
            }
            if b.lat() < min_y {
                min_y = b.lat();
            }
            if a.lon() > max_x {
                max_x = a.lon();
            }
            if b.lon() > max_x {
                max_x = b.lon();
            }
            if a.lat() > max_y {
                max_y = a.lat();
            }
            if b.lat() > max_y {
                max_y = b.lat();
            }
        }

        BoundingBox {
            sw: Point4326::new(min_y, min_x),
            ne: Point4326::new(max_y, max_x),
        }
    }

    /// Render the network as an image.
    pub fn render_image(&self, bounds: BoundingBox, width: u32, height: u32)
        -> tiny_skia::Canvas
    {
        let mut canvas = tiny_skia::Canvas::new(width, height).unwrap();
        canvas.pixmap.fill(tiny_skia::Color::WHITE);

        if self.edges_map.is_empty() {
            return canvas;
        }

        let bounds_3035 = bounds.get_3035_bounds();

        let bounds_width = bounds_3035.ne.coords.0 - bounds_3035.sw.coords.0;
        let bounds_height = bounds_3035.ne.coords.1 - bounds_3035.sw.coords.1;

        let canvas_ratio = width as f64 / height as f64;
        let bounds_ratio = bounds_width / bounds_height;


        let (scale, offset_x, offset_y) = if bounds_ratio > canvas_ratio {
            let scale = width as f64 / bounds_width;
            (
                scale,
                0.0,
                (height as f64 - bounds_height * scale) * 0.5,
            )
        } else {
            let scale = height as f64 / bounds_height;
            (
                scale,
                (width as f64 - bounds_width * scale) * 0.5,
                0.0,
            )
        };

        let max_number = self.edges().max_by_key(|e| e.number).unwrap().number;
        let s2l = |r: u8, g: u8, b: u8| -> palette::LinSrgb {
            palette::Srgb::new(
                r as f32 / 255.0,
                g as f32 / 255.0,
                b as f32 / 255.0,
            ).into_linear()
        };

        // ColorBrewer YlGn color scale
        let colors: Vec<palette::LinSrgb> = vec![
            s2l(247,252,185),
            s2l(217,240,163),
            s2l(173,221,142),
            s2l(120,198,121),
            s2l(65,171,93),
            s2l(35,132,67),
            s2l(0,104,55),
            s2l(0,69,41),
        ];
        let gradient = palette::Gradient::new(colors);
        let mut paint = tiny_skia::Paint::default();
        paint.anti_alias = true;
        let mut stroke = tiny_skia::Stroke::default();
        stroke.width = 4.0;
        stroke.line_cap = tiny_skia::LineCap::Round;

        let mut edges: Vec<_> = self.edges().filter(|e| e.number > 0).collect();
        edges.sort_by_key(|e| e.number);

        for edge in edges {
            let a: Point3035 = edge.a.as_point3035();
            let b: Point3035 = edge.b.as_point3035();
            let path = {
                let mut pb = tiny_skia::PathBuilder::new();
                pb.move_to(
                    (offset_x + (a.coords.0 - bounds_3035.sw.coords.0) * scale) as f32,
                    (offset_y + (bounds_3035.ne.coords.1 - a.coords.1) * scale) as f32,
                );
                pb.line_to(
                    (offset_x + (b.coords.0 - bounds_3035.sw.coords.0) * scale) as f32,
                    (offset_y + (bounds_3035.ne.coords.1 - b.coords.1) * scale) as f32,
                );
                pb.finish().unwrap()
            };

            let c = palette::Srgb::from_linear(gradient.get(edge.number as f32 / max_number as f32));

            paint.set_color(tiny_skia::Color::from_rgba(c.red, c.green, c.blue, 1.0).unwrap());
            canvas.stroke_path(&path, &paint, &stroke);
        }
        canvas
    }

    /// Render an image of the network and save as a PNG file.
    pub fn write_png<P: AsRef<Path>>(&self, path: P, bounds: BoundingBox, width: u32, height: u32)
        -> Result<(), std::io::Error>
    {
        let canvas = self.render_image(bounds, width, height);
        canvas.pixmap.save_png(path)?;
        Ok(())
    }

    pub fn build_polylines(&self) -> PolylineCollection {
        PolylineCollection::new(self)
    }
}
