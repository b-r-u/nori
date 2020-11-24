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

/// An undefined OSM edge.
/// TODO Maybe use NonZeroI64 for OsmNodeId?
pub const UNDEF_OSM_EDGE: (OsmNodeId, OsmNodeId) = (OsmNodeId(0), OsmNodeId(0));

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
    /// OpenStreetMap node ids
    pub osm_ids: (OsmNodeId, OsmNodeId),
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

    pub fn write_to_geojson<P: AsRef<Path>>(&self, output_path: P) -> anyhow::Result<()> {
        let mut writer = GeoJsonWriter::from_path(output_path)?;

        for edge in self.edges() {
            if edge.number < 1 {
                continue;
            }

            let mut ls = writer.add_line_string(&[edge.a, edge.b])?;
            ls.add_property("number", edge.number)?;
            ls.add_property("a_index", edge.a_index)?;
            ls.add_property("b_index", edge.b_index)?;
            ls.finish()?;
        }

        writer.finish()?;

        Ok(())
    }


    pub fn edges(&self) -> impl Iterator<Item=Edge> + '_ {
        self.edges_map.iter().map(move |(&osm_ids, &(a_index, b_index, number))| {
            let source = self.nodes_vec[a_index as usize];
            let target = self.nodes_vec[b_index as usize];
            Edge {
                a: Point4326::new(source.1 as f64 * 0.000001, source.2 as f64 * 0.000001),
                b: Point4326::new(target.1 as f64 * 0.000001, target.2 as f64 * 0.000001),
                number: number,
                a_index,
                b_index,
                osm_ids,
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
            let a: Point3035 = laea::forward(edge.a);
            let b: Point3035 = laea::forward(edge.b);
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
