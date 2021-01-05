use std::collections::{HashMap, HashSet};
use std::path::Path;

use geomatic::Point4326;

use crate::geojson_writer::GeoJsonWriter;
use crate::network::{Network, OsmNodeId};


pub struct PolyPoint {
    pub id: OsmNodeId,
    pub point: Point4326,
}

pub struct Polyline {
    pub points: Vec<PolyPoint>,
}

pub struct PolylineCollection {
    pub polylines: Vec<Polyline>,
    /// Which polyline does the given edge belong to? Returns index into Vec.
    membership: HashMap<(OsmNodeId, OsmNodeId), u32>,
}

impl PolylineCollection {
    /// Split the given network into polylines.
    pub fn new(net: &Network) -> Self {
        let mut poly_collection = PolylineCollection {
            polylines: vec![],
            membership: HashMap::new(),
        };

        // Build adjacency graph
        let adja = {
            let mut adja: HashMap<OsmNodeId, (Point4326, Vec<OsmNodeId>)> = HashMap::new();
            for edge in net.edges() {
                adja.entry(edge.osm_ids().0)
                    .and_modify(|(_, v)| { v.push(edge.osm_ids().1) })
                    .or_insert_with(|| (edge.a.as_point4326(), vec![edge.osm_ids().1]));
                adja.entry(edge.osm_ids().1)
                    .and_modify(|(_, v)| { v.push(edge.osm_ids().0) })
                    .or_insert_with(|| (edge.b.as_point4326(), vec![edge.osm_ids().0]));
            }
            adja
        };

        // Edges that have been processed
        let mut seen: HashSet<(OsmNodeId, OsmNodeId)> = HashSet::new();

        // Build a polyline by following an edge until it ends or reaches an intersection.
        let mut follow = |first_point: Point4326, first_id: OsmNodeId, second_id: OsmNodeId| {
            // Abort if the first segment has already been processed
            let first_segment_seen = seen.contains(&(first_id, second_id)) || seen.contains(&(second_id, first_id));
            if first_segment_seen {
                return;
            } else {
                seen.insert((first_id, second_id));
            }

            // Beware of infinite loops! Each node should only be included once.
            let mut set: HashSet<OsmNodeId> = HashSet::new();
            set.insert(first_id);

            let mut poly = Polyline {
                points: vec![PolyPoint { id: first_id, point: first_point }],
            };

            let poly_id = poly_collection.polylines.len() as u32;
            poly_collection.membership.insert((first_id, second_id), poly_id);

            let mut prev_id = first_id;
            let mut cur_id = second_id;
            let mut cur = &adja[&cur_id];
            loop {
                poly.points.push(PolyPoint{ id: cur_id, point: cur.0 });
                poly_collection.membership.insert((prev_id, cur_id), poly_id);
                seen.insert((prev_id, cur_id));

                if cur.1.len() != 2 {
                    // Stop, the line ends or reaches an intersection.
                    break;
                }
                if set.contains(&cur_id) {
                    // Stop, we reached a loop.
                    break;
                }
                set.insert(cur_id);

                // Get next node
                if cur.1[0] == prev_id {
                    prev_id = cur_id;
                    cur_id = cur.1[1];
                } else {
                    prev_id = cur_id;
                    cur_id = cur.1[0];
                }
                cur = &adja[&cur_id];
            }

            // Add our new polyline.
            poly_collection.polylines.push(poly);
        };

        // Add polylines, starting from nowhere (only one neighbor) or starting from an
        // intersection (more than two neighbors).
        for (id, (point, v)) in &adja {
            if v.len() != 2 {
                for next_id in v {
                    follow(*point, *id, *next_id);
                }
            }
        }

        // Add missing polylines that have no clear start or end.
        for (id, (point, v)) in &adja {
            if v.len() == 2 {
                follow(*point, *id, v[0]);
                follow(*point, *id, v[1]);
            }
        }

        poly_collection
    }

    /// Return the id of the polyline that contains the given edge.
    pub fn lookup_edge(&self, edge: (OsmNodeId, OsmNodeId)) -> Option<u32> {
        // search edge and reversed edge
        self.membership.get(&edge)
            .or_else(|| self.membership.get(&(edge.1, edge.0)))
            .cloned()
    }

    pub fn write_to_geojson<P: AsRef<Path>>(&self, output_path: P) -> anyhow::Result<()> {
        let mut writer = GeoJsonWriter::from_path(output_path)?;

        for (i, poly) in self.polylines.iter().enumerate() {
            let coords: Vec<_> = poly.points.iter().map(|p| p.point).collect();
            let mut ls = writer.add_line_string(&coords)?;
            ls.add_property("id", i)?;
            ls.finish()?;
        }

        writer.finish()?;

        Ok(())
    }
}
