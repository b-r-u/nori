use std::fs::File;
use std::io::BufReader;
use std::path::Path;

use geomatic::{laea, Point4326, Point3035};
use rand::Rng;
use rand::prelude::*;
use rand::distributions::WeightedIndex;

use crate::bounding_box::BoundingBox;


pub trait Sampling {
    fn gen_point(&mut self) -> Point4326;
}

pub struct Uniform2D {
    rng: rand::rngs::ThreadRng,
    bounds: BoundingBox,
}

impl Uniform2D {
    pub fn new(bounds: BoundingBox) -> Self {
        Uniform2D {
            rng: rand::thread_rng(),
            bounds,
        }
    }
}

impl Sampling for Uniform2D {
    fn gen_point(&mut self) -> Point4326 {
        let min_lat = self.bounds.sw.lat();
        let min_lon = self.bounds.sw.lon();
        let max_lat = self.bounds.ne.lat();
        let max_lon = self.bounds.ne.lon();
        let lon: f64 = self.rng.gen::<f64>() * (max_lon - min_lon) + min_lon;
        let lat: f64 = self.rng.gen::<f64>() * (max_lat - min_lat) + min_lat;
        Point4326::new(lat, lon)
    }
}


pub struct Weighted {
    rng: rand::rngs::ThreadRng,
    dist: rand::distributions::weighted::WeightedIndex<u32>,
    points: Vec<Point4326>,
}

impl Weighted {
    pub fn from_csv<P: AsRef<Path>>(path: P, bounds: Option<BoundingBox>) -> Result<Self, Box<dyn std::error::Error>> {
        println!("READ CSV");
        let buf_reader = BufReader::new(File::open(path)?);
        let mut rdr = csv::ReaderBuilder::new()
            .has_headers(true)
            .from_reader(buf_reader);
        println!("start reading...");
        let mut weights: Vec<_> = vec![];
        let mut points: Vec<Point4326> = vec![];
        for result in rdr.records() {
            let record = result?;
            assert_eq!(record.len(), 3);
            let x: f64 = record.get(0).unwrap().parse()?;
            let y: f64 = record.get(1).unwrap().parse()?;
            let weight: u32 = record.get(2).unwrap().parse()?;
            let p = Point3035::new(x, y);
            let p: Point4326 = laea::backward(p);
            if bounds.is_none() || bounds.unwrap().is_inside(p) {
                weights.push(weight);
                points.push(p);
            }
        }
        println!("done reading");
        let dist = WeightedIndex::new(&weights).unwrap();
        Ok(Weighted {
            rng: rand::thread_rng(),
            dist,
            points,
        })
    }
}

impl Sampling for Weighted {
    fn gen_point(&mut self) -> Point4326 {
        self.points[self.dist.sample(&mut self.rng)]
    }
}
