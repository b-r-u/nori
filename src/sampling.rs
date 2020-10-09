use std::fs::File;
use std::io::BufReader;
use std::path::Path;

use geomatic::{laea, Point4326, Point3035};
use kdtree::KdTree;
use kdtree::distance::squared_euclidean;
use rand::Rng;
use rand::distributions::WeightedIndex;
use rand::prelude::*;

use crate::bounding_box::BoundingBox;


pub trait Sampling {
    fn gen_source(&mut self) -> Point4326;
    fn gen_destination(&mut self, source: Point4326) -> Point4326;
}

pub struct Uniform2D {
    rng: rand::rngs::ThreadRng,
    bounds: BoundingBox,
    max_dist: f64,
}

impl Uniform2D {
    pub fn new(bounds: BoundingBox, max_dist: f64) -> Self {
        Uniform2D {
            rng: rand::thread_rng(),
            bounds,
            max_dist,
        }
    }

    /// Random sample inside a circle of the given radius. (Thanks SO!)
    fn sample_circle(&mut self, radius: f64) -> (f64, f64) {
        let angle = (2.0 * std::f64::consts::PI) * self.rng.gen::<f64>();
        let u = self.rng.gen::<f64>() + self.rng.gen::<f64>();
        let r = radius * (if u > 1.0 { 2.0 - u } else { u });
        (r * angle.cos(), r * angle.sin())
    }
}

impl Sampling for Uniform2D {
    fn gen_source(&mut self) -> Point4326 {
        let min_lat = self.bounds.sw.lat();
        let min_lon = self.bounds.sw.lon();
        let max_lat = self.bounds.ne.lat();
        let max_lon = self.bounds.ne.lon();
        let lon: f64 = self.rng.gen::<f64>() * (max_lon - min_lon) + min_lon;
        let lat: f64 = self.rng.gen::<f64>() * (max_lat - min_lat) + min_lat;
        Point4326::new(lat, lon)
    }

    fn gen_destination(&mut self, source: Point4326) -> Point4326 {
        let delta = self.sample_circle(self.max_dist);
        let p = laea::forward(source);
        let p = Point3035::new(p.coords.0 + delta.0, p.coords.1 + delta.1);
        laea::backward(p)
    }
}


pub struct Weighted {
    rng: rand::rngs::ThreadRng,
    dist: rand::distributions::weighted::WeightedIndex<u32>,
    points: Vec<Point4326>,
    weights: Vec<u32>,
    /// A k-d tree with indices into `points` and `weights`.
    kdtree: KdTree<f64, usize, [f64;2]>,
    /// maximum distance in meters between source and destination points.
    max_dist: f64,
}

impl Weighted {
    pub fn from_csv<P: AsRef<Path>>(path: P, bounds: Option<BoundingBox>, max_dist: f64) -> Result<Self, Box<dyn std::error::Error>> {
        println!("READ CSV");
        let buf_reader = BufReader::new(File::open(path)?);
        let mut rdr = csv::ReaderBuilder::new()
            .has_headers(true)
            .from_reader(buf_reader);
        println!("start reading...");
        let mut weights: Vec<_> = vec![];
        let mut points: Vec<Point4326> = vec![];
        let mut kdtree = KdTree::new(2);
        for result in rdr.records() {
            let record = result?;
            assert_eq!(record.len(), 3);
            let x: f64 = record.get(0).unwrap().parse()?;
            let y: f64 = record.get(1).unwrap().parse()?;
            let weight: u32 = record.get(2).unwrap().parse()?;
            let p3035 = Point3035::new(x, y);
            let p4326: Point4326 = laea::backward(p3035);
            if bounds.is_none() || bounds.unwrap().is_inside(p4326) {
                kdtree.add([p3035.coords.0, p3035.coords.1], points.len()).unwrap();
                weights.push(weight);
                points.push(p4326);
            }
        }
        println!("done reading");
        let dist = WeightedIndex::new(&weights).unwrap();
        Ok(Weighted {
            rng: rand::thread_rng(),
            dist,
            points,
            weights,
            kdtree,
            max_dist,
        })
    }
}

impl Sampling for Weighted {
    fn gen_source(&mut self) -> Point4326 {
        self.points[self.dist.sample(&mut self.rng)]
    }

    fn gen_destination(&mut self, source: Point4326) -> Point4326 {
        let source = laea::forward(source);

        // Get point indices within max_distance.
        // Errors only if the dimension is wrong or a coordinate is not finite.
        let indices = self.kdtree.within(
            &[source.coords.0, source.coords.1],
            // square distance
            self.max_dist.powi(2),
            &squared_euclidean,
        ).unwrap();

        let distribution = WeightedIndex::new(indices.iter().map(|p| self.weights[*p.1])).unwrap();
        let selected_index = indices[distribution.sample(&mut self.rng)];
        let destination = self.points[*selected_index.1];

        destination
    }
}
