//! Compare generated traffic numbers with empirical data.

use std::fs::File;
use std::io::Read;
use std::path::Path;

use anyhow::Context;
use geojson::GeoJson;
use geomatic::{laea, Point3035, Point4326};
use rstar::primitives::Line;
use rstar::{AABB, PointDistance, RTree, RTreeObject};

use crate::geojson_writer::GeoJsonWriter;
use crate::network;


/// A line segment that can be inserted into an RTree.
pub struct Segment {
    line: Line<[f64; 2]>,
    number: f64,
}

impl Segment {
    fn new(a: Point3035, b: Point3035, number: f64) -> Self {
        Segment {
            line: Line::new([a.coords.0, a.coords.1], [b.coords.0, b.coords.1]),
            number,
        }
    }
}

impl RTreeObject for Segment
{
    type Envelope = AABB<[f64; 2]>;

    fn envelope(&self) -> Self::Envelope
    {
        self.line.envelope()
    }
}

impl PointDistance for Segment
{
    fn distance_2(&self, point: &[f64; 2]) -> f64
    {
        self.line.distance_2(point)
    }

    fn contains_point(&self, point: &[f64; 2]) -> bool
    {
        self.line.contains_point(point)
    }
}

fn orientation(line: &Line<[f64; 2]>) -> f64 {
    let dx = line.to[0] - line.from[0];
    let dy = line.to[1] - line.from[1];
    // angle in [-pi, pi]
    let angle = dy.atan2(dx);
    if angle >= 0.0 {
        angle
    } else {
        angle + std::f64::consts::PI
    }
}

/// Return the difference in orientation in radians.
fn orientation_diff(line_a: &Line<[f64; 2]>, line_b: &Line<[f64; 2]>) -> f64 {
    let mut orient_a = orientation(line_a);
    let mut orient_b = orientation(line_b);
    if orient_a > orient_b {
        std::mem::swap(&mut orient_a, &mut orient_b);
    }
    let diff_1 = orient_b - orient_a;
    let diff_2 = orient_a + std::f64::consts::PI - orient_b;
    diff_1.min(diff_2)
}

fn line_center(line: &Line<[f64; 2]>) -> [f64;2] {
    [
        0.5 * (line.from[0] + line.to[0]),
        0.5 * (line.from[1] + line.to[1]),
    ]
}

fn edge_as_line(edge: &network::Edge) -> Line<[f64; 2]> {
    let a = laea::forward(edge.a);
    let b = laea::forward(edge.b);
    Line::new([a.coords.0, a.coords.1], [b.coords.0, b.coords.1])
}

pub fn compare<P: AsRef<Path>>(net: &network::Network, geojson_path: P, number_property: &str)
    -> anyhow::Result<()>
{
    let reference_traffic = geojson_to_rtree(&geojson_path, number_property)
        .with_context(
            || format!("Failed to read GeoJSON file {:?}", geojson_path.as_ref().display())
        )?;
    println!("Built reference RTree with {} segments", reference_traffic.size());
    let simulated_traffic = network_to_rtree(net);
    println!("Built simulated RTree with {} segments", simulated_traffic.size());

    let mut writer = GeoJsonWriter::from_path("connections.geojson")?;

    for edge in net.edges() {
        if edge.number == 0 {
            continue;
        }
        let line = edge_as_line(&edge);
        let center = line_center(&line);
        for (nn, dist_2) in reference_traffic.nearest_neighbor_iter_with_distance_2(&center).take(1) {
            if dist_2 > 20_f64.powi(2) {
                break;
            }
            let orient = orientation_diff(&line, &nn.line);

            if orient < 0.01 {
                let from = laea::backward(Point3035::new(center[0], center[1]));
                let np = nn.line.nearest_point(&center);
                let to = laea::backward(Point3035::new(np[0], np[1]));
                {
                    let mut feat = writer.add_line_string(from, to)?;
                    feat.add_property("number_empir", nn.number)?;
                    feat.add_property("number_sim", edge.number)?;
                    feat.add_property("length", dist_2.sqrt())?;
                    feat.finish()?;
                }
                break;
            }
        }
    }

    writer.finish()?;

    Ok(())
}

pub fn network_to_rtree(network: &network::Network) -> RTree<Segment> {
    let segments: Vec<Segment> = network.edges()
        .filter(|edge| edge.number > 0)
        .map(|edge| Segment::new(
            laea::forward(edge.a),
            laea::forward(edge.b),
            edge.number as f64,
        ))
        .collect();

    let tree = RTree::bulk_load(segments);
    tree
}

pub fn geojson_to_rtree<P: AsRef<Path>>(geojson_path: P, number_property: &str)
    -> anyhow::Result<RTree<Segment>>
{
    // Parse GeoJSON
    let geojson: GeoJson = {
        let mut f = File::open(&geojson_path)?;
        let mut geojson_str = String::new();
        f.read_to_string(&mut geojson_str)?;
        geojson_str.parse::<GeoJson>()?
    };

    // line segments of geojson file
    let mut segments: Vec<Segment> = vec![];

    // Gather segments of all line strings
    match &geojson {
        &GeoJson::FeatureCollection(ref fc) => {
            for feature in &fc.features {
                let line_string = feature.geometry.as_ref().map(|g| &g.value);
                let number = feature.properties.as_ref().and_then(|p| p.get(number_property)).and_then(|n| n.as_f64());

                if let (Some(geojson::Value::LineString(line_string)), Some(number)) = (line_string, number) {
                    let mut last_point = None;
                    for point in line_string {
                        if point.len() >= 2 {
                            let current_point = laea::forward(Point4326::new(point[1], point[0]));
                            if let Some(last_point) = last_point {
                                segments.push(Segment::new(last_point, current_point, number));
                            }
                            last_point = Some(current_point);
                        }
                    }
                }
            }
        },
        _ => anyhow::bail!("GeoJSON file is no FeatureCollection: {}", geojson_path.as_ref().display()),
    }

    if segments.is_empty() {
        anyhow::bail!("GeoJSON file contains no features with the given property {:?}", number_property);
    }

    // Build RTree
    let tree = RTree::bulk_load(segments);
    Ok(tree)
}


#[cfg(test)]
mod tests {
    use super::*;
    use std::f64::consts::PI;

    fn approx_eq(a: f64, b: f64) -> bool {
        (a - b).abs() < 1.0e-10
    }

    #[test]
    fn test_orientation() {
        let a = Line::new([0.0, 0.0], [1.0, 0.0]);
        let b = Line::new([0.0, 0.0], [0.0, 1.0]);
        assert!(approx_eq(orientation(&a), 0.0));
        assert!(approx_eq(orientation(&b), PI * 0.5));
        assert!(approx_eq(orientation_diff(&a, &b), PI * 0.5));
        assert!(approx_eq(orientation_diff(&b, &a), PI * 0.5));
        let a2 = Line::new([1.0, 0.0], [0.0, 0.0]);
        assert!(approx_eq(orientation_diff(&a2, &b), PI * 0.5));
        assert!(approx_eq(orientation_diff(&b, &a2), PI * 0.5));
    }
}
