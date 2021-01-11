use std::path::Path;
use std::collections::HashMap;

use geomatic::{laea, Point3035, Point4326};
use osmpbf::{Element, IndexedReader};
use serde::Serialize;


#[derive(Serialize)]
struct CsvRecord {
    // x coordinate of raster center, EPSG:3035
    x_mp_100m: u32,
    // y coordinate of raster center, EPSG:3035
    y_mp_100m: u32,
    // weight of this cell
    weight: f32,
}

#[derive(Copy, Clone, Debug, Eq, Hash, PartialEq)]
struct GridCell100m {
    x_center: u32,
    y_center: u32,
}

impl GridCell100m {
    fn from_point4326(point: Point4326) -> Self {
        let point: Point3035 = laea::forward(point);
        GridCell100m {
            x_center: (point.east() * 0.01).floor() as u32 * 100 + 50,
            y_center: (point.north() * 0.01).floor() as u32 * 100 + 50,
        }
    }
}

pub fn filter_poi<P: AsRef<Path>, Q: AsRef<Path>>(osmpbf_path: P, csv_output_path: Q)
    -> anyhow::Result<()>
{
    let mut reader = IndexedReader::from_path(&osmpbf_path)?;
    let mut csv_writer = csv::Writer::from_path(csv_output_path)?;

    let mut cells = HashMap::<GridCell100m, u32>::new();

    {
        let mut nodes = HashMap::<i64, Point4326>::new();
        let mut ways = Vec::<Vec<i64>>::new();

        reader.read_ways_and_deps(
            |way| {
                // Filter ways.
                way.tags().any(|key_value| key_value == ("shop", "supermarket"))
            },
            |element| {
                // Increment counter for ways and nodes
                match element {
                    Element::Way(way) => {
                        ways.push(way.refs().collect());
                    },
                    Element::Node(node) => {
                        nodes.insert(node.id(), Point4326::new(node.lat(), node.lon()));
                    },
                    Element::DenseNode(dense_node) => {
                        nodes.insert(dense_node.id, Point4326::new(dense_node.lat(), dense_node.lon()));
                    },
                    Element::Relation(_) => {} // should not occur
                }
            },
        )?;

        for way in ways {
            // compute centroid of ways
            let factor = (way.len() as f64).recip();
            let lat = way.iter().map(|node_idx| nodes[node_idx].lat()).sum::<f64>() * factor;
            let lon = way.iter().map(|node_idx| nodes[node_idx].lon()).sum::<f64>() * factor;
            let centroid = Point4326::new(lat, lon);
            // Insert/update grid cell
            let grid_cell = GridCell100m::from_point4326(centroid);
            *cells.entry(grid_cell).or_insert(0) += 1;
        }
    }

    reader.for_each_node(
        |element| {
            match element {
                Element::Node(node) => {
                    if node.tags().any(|key_value| key_value == ("shop", "supermarket")) {
                        let point = Point4326::new(node.lat(), node.lon());
                        // Insert/update grid cell
                        let grid_cell = GridCell100m::from_point4326(point);
                        *cells.entry(grid_cell).or_insert(0) += 1;
                    }
                },
                Element::DenseNode(dense_node) => {
                    if dense_node.tags().any(|key_value| key_value == ("shop", "supermarket")) {
                        let point = Point4326::new(dense_node.lat(), dense_node.lon());
                        // Insert/update grid cell
                        let grid_cell = GridCell100m::from_point4326(point);
                        *cells.entry(grid_cell).or_insert(0) += 1;
                    }
                }
                _ => {},
            }
        },
    )?;

    for (cell, &weight) in cells.iter() {
        csv_writer.serialize(CsvRecord {
            x_mp_100m: cell.x_center,
            y_mp_100m: cell.y_center,
            weight: weight as f32,
        })?;
    }

    Ok(())
}
