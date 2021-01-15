use std::path::Path;

use geomatic::{laea, Point4326, Point3035};
use rand::prelude::*;

use crate::bounding_box::BoundingBox;
use crate::density::DensityClusters;


pub trait Sampling {
    fn gen_source(&mut self) -> Point4326;
    fn gen_destination(&mut self, source: Point4326) -> Option<Point4326>;
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

    fn gen_destination(&mut self, source: Point4326) -> Option<Point4326> {
        let delta = self.sample_circle(self.max_dist);
        let p = laea::forward(source);
        let p = Point3035::new(p.coords.0 + delta.0, p.coords.1 + delta.1);
        Some(laea::backward(p))
    }
}


pub struct Weighted {
    rng: rand::rngs::ThreadRng,
    density: DensityClusters,
    max_dist: f64,
}

impl Weighted {
    pub fn from_csv<P: AsRef<Path>>(path: P, bounds: Option<BoundingBox>, max_dist: f64)
        -> anyhow::Result<Self>
    {
        Ok(Weighted {
            rng: rand::thread_rng(),
            density: DensityClusters::from_csv(path, bounds)?,
            max_dist,
        })
    }
}

impl Sampling for Weighted {
    fn gen_source(&mut self) -> Point4326 {
        self.density.sample_point(&mut self.rng)
    }

    fn gen_destination(&mut self, source: Point4326) -> Option<Point4326> {
        self.density.sample_point_within(&mut self.rng, source, self.max_dist)
    }
}


pub struct Complex {
    rng: rand::rngs::ThreadRng,
    /// maximum distance in meters between source and destination points.
    max_dist: f64,
    density_population: DensityClusters,
    density_poi: DensityClusters,
}

impl Complex {
    pub fn from_csv<P, Q>(population_csv: P, poi_csv: Q, bounds: Option<BoundingBox>, max_dist: f64)
        -> anyhow::Result<Self>
        where
            P: AsRef<Path>,
            Q: AsRef<Path>,
    {
        Ok(Complex {
            rng: rand::thread_rng(),
            max_dist,
            density_population: DensityClusters::from_csv(population_csv, bounds)?,
            density_poi: DensityClusters::from_csv(poi_csv, bounds)?,
        })
    }
}

impl Sampling for Complex {
    fn gen_source(&mut self) -> Point4326 {
        if self.rng.gen::<bool>() {
            self.density_population.sample_point(&mut self.rng)
        } else {
            self.density_poi.sample_point(&mut self.rng)
        }
    }

    fn gen_destination(&mut self, source: Point4326) -> Option<Point4326> {
        if self.rng.gen::<bool>() {
            self.density_population.sample_point_within(&mut self.rng, source, self.max_dist)
        } else {
            self.density_poi.sample_point_within(&mut self.rng, source, self.max_dist)
        }
    }
}
