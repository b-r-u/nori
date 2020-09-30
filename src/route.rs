use serde::{Serialize, Deserialize};
use std::io::{BufReader, BufWriter, Read, Write};
use std::fs::File;
use std::path::Path;
use bincode;

type LatLon = (i32, i32);

#[derive(Serialize, Deserialize, PartialEq, Debug)]
pub struct Route {
    pub start_coord: LatLon,
    pub end_coord: LatLon,
    pub node_ids: Vec<i64>,
}

#[derive(Serialize, Deserialize, PartialEq, Debug)]
struct RouteCollectionHeader {
    major_version: u16,
    minor_version: u16,
    osrm_file: String,
    scenario: String,
    number_of_routes: u64,
}

pub struct RouteCollectionWriter<W: Write> {
    writer: BufWriter<W>,
    number_of_routes: u64,
}


impl RouteCollectionWriter<File> {
    pub fn new<P: AsRef<Path>, S: Into<String>>(path: P, osrm_file: S, scenario: S) -> Result<RouteCollectionWriter<File>, Box<dyn std::error::Error>> {
        let mut writer = BufWriter::new(File::create(path)?);

        // write header
        let header = RouteCollectionHeader {
            major_version: 0,
            minor_version: 1,
            osrm_file: osrm_file.into(),
            scenario: scenario.into(),
            number_of_routes: 0,
        };
        bincode::serialize_into(&mut writer, &header)?;

        Ok(RouteCollectionWriter {
            writer,
            number_of_routes: 0,
        })
    }

    pub fn write_route(&mut self, route: Route) -> Result<Route, Box<dyn std::error::Error>> {
        bincode::serialize_into(&mut self.writer, &route)?;
        self.number_of_routes += 1;
        Ok(route)
    }
}

pub struct RouteCollectionReader<R: Read> {
    reader: BufReader<R>,
    header: RouteCollectionHeader,
}

impl RouteCollectionReader<File> {
    pub fn new<P: AsRef<Path>, S: Into<String>>(path: P) -> Result<RouteCollectionReader<File>, Box<dyn std::error::Error>> {
        let mut reader = BufReader::new(File::open(path)?);

        // read header
        let header = bincode::deserialize_from(&mut reader)?;

        Ok(RouteCollectionReader {
            reader,
            header,
        })
    }

    pub fn read_route(&mut self) -> Result<Route, Box<dyn std::error::Error>> {
        Ok(bincode::deserialize_from(&mut self.reader)?)
    }
}

