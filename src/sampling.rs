use std::path::Path;
use rand::Rng;


pub trait Sampling {
    fn gen_point(&mut self) -> (f64, f64);
}

pub struct Uniform2D {
    rng: rand::rngs::ThreadRng,
    min_lon: f64,
    min_lat: f64,
    max_lon: f64,
    max_lat: f64,
}

impl Uniform2D {
    pub fn new(min_lon: f64, min_lat: f64, max_lon: f64, max_lat: f64) -> Self {
        Uniform2D {
            rng: rand::thread_rng(),
            min_lon,
            min_lat,
            max_lon,
            max_lat,
        }
    }
}

impl Sampling for Uniform2D {
    fn gen_point(&mut self) -> (f64, f64) {
        let lon: f64 = self.rng.gen::<f64>() * (self.max_lon - self.min_lon) + self.min_lon;
        let lat: f64 = self.rng.gen::<f64>() * (self.max_lat - self.min_lat) + self.min_lat;
        (lon, lat)
    }
}



pub struct Weighted {
}

impl Weighted {
    pub fn from_csv<P: AsRef<Path>>(path: P) -> Result<Self, Box<dyn std::error::Error>> {
        Ok(
            Weighted {}
        )
    }
}

/*
impl Sampling for Weighted {
    fn gen_point(&mut self) -> (f64, f64) {
    }
}
*/
