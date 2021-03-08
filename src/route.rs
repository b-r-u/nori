use std::io::{BufReader, BufWriter, Read, Seek, Write};
use std::fs::File;
use std::path::Path;

use bincode;
use geomatic::{laea, Point4326};
use serde::{Serialize, Deserialize};

use crate::geojson_writer::GeoJsonWriter;
use crate::network::{Network, OsmNodeId};


#[derive(Serialize, Deserialize, PartialEq, Debug)]
pub struct LatLon32 {
    raw_lat: i32,
    raw_lon: i32,
}

impl LatLon32 {
    pub fn new(lat: f64, lon: f64) -> Self {
        LatLon32 {
            raw_lat: (lat * 1e6) as i32,
            raw_lon: (lon * 1e6) as i32,
        }
    }

    fn as_point4326(&self) -> Point4326 {
        Point4326::new(
            (self.raw_lat as f64) * 1e-6,
            (self.raw_lon as f64) * 1e-6,
        )
    }
}

#[derive(Serialize, Deserialize, PartialEq, Debug)]
pub struct Route {
    pub start_coord: LatLon32,
    pub end_coord: LatLon32,
    pub node_ids: Vec<OsmNodeId>,
    pub distance: f64,
}

impl Route {
    /// Distance of straight line between start point and end point of the projected coordinates.
    pub fn distance_bee_line(&self) -> f64 {
        let a = laea::forward(self.start_coord.as_point4326());
        let b = laea::forward(self.end_coord.as_point4326());
        let dx = a.coords.0 - b.coords.0;
        let dy = a.coords.1 - b.coords.1;
        dx.hypot(dy)
    }

    pub fn write_to_geojson<P: AsRef<Path>>(&self, output_path: P, network: &Network) -> anyhow::Result<()> {
        let mut writer = GeoJsonWriter::from_path(output_path)?;

        let coords: Option<Vec<_>> = self.node_ids.iter().map(|&n| network.get_node(n).map(|n| n.as_point4326())).collect();
        let coords = coords.unwrap();
        let mut ls = writer.add_line_string(&coords)?;
        ls.add_property("distance", self.distance)?;
        ls.add_property("distance_bee_line", self.distance_bee_line())?;
        ls.finish()?;

        writer.finish()?;

        Ok(())
    }
}

#[derive(Serialize, Deserialize, PartialEq, Debug)]
pub struct RouteCollectionHeader {
    major_version: u16,
    minor_version: u16,
    pub osrm_file: String,
    scenario: String,
    number_of_routes: u64,
}

pub struct RouteCollectionWriter<W: Write> {
    writer: BufWriter<W>,
    header: RouteCollectionHeader,
}


impl RouteCollectionWriter<File> {
    pub fn new<P: AsRef<Path>, S: Into<String>>(path: P, osrm_file: S, scenario: S)
        -> anyhow::Result<RouteCollectionWriter<File>>
    {
        let mut writer = BufWriter::new(File::create(path)?);

        // write header
        let header = RouteCollectionHeader {
            major_version: 0,
            minor_version: 2,
            osrm_file: osrm_file.into(),
            scenario: scenario.into(),
            number_of_routes: 0,
        };
        bincode::serialize_into(&mut writer, &header)?;

        Ok(RouteCollectionWriter {
            writer,
            header,
        })
    }

    pub fn write_route(&mut self, route: Route) -> anyhow::Result<Route> {
        bincode::serialize_into(&mut self.writer, &route)?;
        self.header.number_of_routes += 1;
        Ok(route)
    }

    pub fn finish(mut self) -> anyhow::Result<()> {
        // Move to start of file
        self.writer.seek(std::io::SeekFrom::Start(0))?;
        // Write header again, but with correct number_of_routes
        bincode::serialize_into(&mut self.writer, &self.header)?;
        // Always flush!
        self.writer.flush()?;
        Ok(())
    }
}

pub struct RouteCollectionReader<R: Read> {
    reader: BufReader<R>,
    header: RouteCollectionHeader,
    route_index: u64,
}

impl RouteCollectionReader<File> {
    pub fn new<P: AsRef<Path>>(path: P) -> anyhow::Result<RouteCollectionReader<File>> {
        let mut reader = BufReader::new(File::open(path)?);

        // read header
        let header = bincode::deserialize_from(&mut reader)?;

        Ok(RouteCollectionReader {
            reader,
            header,
            route_index: 0,
        })
    }

    pub fn header(&self) -> &RouteCollectionHeader {
        &self.header
    }
}

impl Iterator for RouteCollectionReader<File> {
    type Item = anyhow::Result<Route>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.route_index >= self.header.number_of_routes {
            None
        } else {
            self.route_index += 1;
            match bincode::deserialize_from(&mut self.reader) {
                Ok(route) => Some(Ok(route)),
                Err(err) => Some(Err(err.into())),
            }
        }
    }
}
