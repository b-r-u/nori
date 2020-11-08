use std::collections::HashMap;
use std::fs::File;
use std::io::{BufWriter, Write};
use std::path::Path;

use geomatic::{laea, Point3035, Point4326};
use osrmreader::{Entry, OsrmReader};
use raqote::DrawTarget;
use serde::{Serialize, Deserialize};

use crate::bounding_box::BoundingBox;


#[derive(Copy, Clone, Debug, Eq, Hash, PartialEq, Serialize, Deserialize)]
pub struct OsmNodeId(i64);

pub struct Network {
    nodes_vec: Vec<(OsmNodeId, i32, i32)>,
    edges_map: HashMap<(OsmNodeId, OsmNodeId), (u32, u32, usize)>,
}

pub struct Edge {
    /// first point
    pub a: Point4326,
    /// second point
    pub b: Point4326,
    /// Number of routes that passed trough this edge
    pub number: usize,
    /// Index of first point into node vector
    pub a_index: u32,
    /// Index of second point
    pub b_index: u32,
}

impl Network {
    pub fn bump_edges(&mut self, nodes: &[OsmNodeId]) {
        for win in nodes.windows(2) {
            if win.len() == 2 {
                match self.edges_map.get_mut(&(win[0], win[1])) {
                    Some(val) => val.2 += 1,
                    None => {},//println!("lookup fail ({}, {})", win[0], win[1]),
                }
            }
        }
    }

    pub fn write_to_geojson<P: AsRef<Path>>(&self, output_path: P) -> Result<(), std::io::Error> {
        let mut output = BufWriter::new(File::create(output_path)?);
        output.write(b"{\"type\": \"FeatureCollection\", \"features\": [")?;

        let mut first = true;
        for edge in self.edges() {
            if edge.number < 1 {
                continue;
            }

            if !first {
                write!(output, ",")?;
            }
            first = false;

            write!(
                output,
                "\n{{\"type\": \"Feature\", \
                   \"properties\": {{\
                   \"number\": {number}, \
                   \"a_index\": {a_index}, \
                   \"b_index\": {b_index}\
                   }}, \
                   \"geometry\": {{\
                     \"type\": \"LineString\", \
                     \"coordinates\": [[{a_lon:.6}, {a_lat:.6}], [{b_lon:.6}, {b_lat:.6}]]}}}}",
                number = edge.number,
                a_lon = edge.a.lon(),
                a_lat = edge.a.lat(),
                b_lon = edge.b.lon(),
                b_lat = edge.b.lat(),
                a_index = edge.a_index,
                b_index = edge.b_index,
            )?;
        }

        // Properly end the GeoJSON file
        output.write(b"\n]}")?;
        output.flush()?;

        Ok(())
    }


    pub fn edges(&self) -> impl Iterator<Item=Edge> + '_ {
        self.edges_map.values().map(move |&(a_index, b_index, number)| {
            let source = self.nodes_vec[a_index as usize];
            let target = self.nodes_vec[b_index as usize];
            Edge {
                a: Point4326::new(source.1 as f64 * 0.000001, source.2 as f64 * 0.000001),
                b: Point4326::new(target.1 as f64 * 0.000001, target.2 as f64 * 0.000001),
                number: number,
                a_index,
                b_index,
            }
        })
    }


    pub fn from_path<P: AsRef<Path>>(path: P) -> Result<Network, std::io::Error> {
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
                        nodes_vec.push((OsmNodeId(n.node_id), n.raw_latitude, n.raw_longitude))
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

    pub fn get_bounds(&self) -> BoundingBox {
        let mut edges_iter = self.edges();

        let mut min_x = 0.0;
        let mut max_x = 0.0;
        let mut min_y = 0.0;
        let mut max_y = 0.0;

        if let Some(edge) = edges_iter.next() {
            min_x = edge.a.lon().min(edge.b.lon());
            min_y = edge.a.lat().min(edge.b.lat());
            max_x = edge.a.lon().max(edge.b.lon());
            max_y = edge.a.lat().max(edge.b.lat());
        }

        for edge in edges_iter {
            if edge.a.lon() < min_x {
                min_x = edge.a.lon();
            }
            if edge.b.lon() < min_x {
                min_x = edge.b.lon();
            }
            if edge.a.lat() < min_y {
                min_y = edge.a.lat();
            }
            if edge.b.lat() < min_y {
                min_y = edge.b.lat();
            }
            if edge.a.lon() > max_x {
                max_x = edge.a.lon();
            }
            if edge.b.lon() > max_x {
                max_x = edge.b.lon();
            }
            if edge.a.lat() > max_y {
                max_y = edge.a.lat();
            }
            if edge.b.lat() > max_y {
                max_y = edge.b.lat();
            }
        }

        BoundingBox {
            sw: Point4326::new(min_y, min_x),
            ne: Point4326::new(max_y, max_x),
        }
    }

    /// Render the network as an image.
    pub fn render_image(&self, bounds: BoundingBox, width: i32, height: i32)
        -> DrawTarget
    {
        let mut dt = DrawTarget::new(width, height);
        dt.fill_rect(
            0.0,
            0.0,
            width as f32,
            height as f32,
            &raqote::Source::Solid(raqote::SolidSource {
                r: 0xff,
                g: 0xff,
                b: 0xff,
                a: 0xff,
            }),
            &raqote::DrawOptions::new()
        );

        if self.edges_map.is_empty() {
            return dt;
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

        let mut edges: Vec<_> = self.edges().filter(|e| e.number > 0).collect();
        edges.sort_by_key(|e| e.number);

        for edge in edges {
            let a: Point3035 = laea::forward(edge.a);
            let b: Point3035 = laea::forward(edge.b);
            let mut pb = raqote::PathBuilder::new();
            pb.move_to(
                (offset_x + (a.coords.0 - bounds_3035.sw.coords.0) * scale) as f32,
                (offset_y + (bounds_3035.ne.coords.1 - a.coords.1) * scale) as f32,
            );
            pb.line_to(
                (offset_x + (b.coords.0 - bounds_3035.sw.coords.0) * scale) as f32,
                (offset_y + (bounds_3035.ne.coords.1 - b.coords.1) * scale) as f32,
            );
            let path = pb.finish();

            let c = palette::Srgb::from_linear(gradient.get(edge.number as f32 / max_number as f32));

            dt.stroke(
                &path,
                &raqote::Source::Solid(raqote::SolidSource {
                    r: (c.red * 255.0) as u8,
                    g: (c.green * 255.0) as u8,
                    b: (c.blue * 255.0) as u8,
                    a: 0xff,
                }),
                &raqote::StrokeStyle {
                    cap: raqote::LineCap::Round,
                    join: raqote::LineJoin::Round,
                    width: 2.0,
                    miter_limit: 2.0,
                    dash_array: vec![],
                    dash_offset: 0.0,
                },
                &raqote::DrawOptions::new()
            );
        }
        dt
    }

    /// Render an image of the network and save as a PNG file.
    pub fn write_png<P: AsRef<Path>>(&self, path: P, bounds: BoundingBox, width: i32, height: i32)
        -> Result<(), std::io::Error>
    {
        let dt = self.render_image(bounds, width, height);
        dt.write_png(path)?;
        Ok(())
    }
}
