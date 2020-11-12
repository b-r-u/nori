use std::io::{BufReader, BufWriter, Read, Seek, Write};
use std::fs::File;
use std::path::Path;

use bincode;
use serde::{Serialize, Deserialize};

use crate::network::OsmNodeId;

type LatLon = (i32, i32);

#[derive(Serialize, Deserialize, PartialEq, Debug)]
pub struct Route {
    pub start_coord: LatLon,
    pub end_coord: LatLon,
    pub node_ids: Vec<OsmNodeId>,
    pub distance: f64,
}

#[derive(Serialize, Deserialize, PartialEq, Debug)]
pub struct RouteCollectionHeader {
    major_version: u16,
    minor_version: u16,
    osrm_file: String,
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
