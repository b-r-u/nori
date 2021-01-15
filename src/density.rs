//! Represent spatial density by weighted clusters

use std::fs::File;
use std::io::BufReader;
use std::path::Path;

use geomatic::{laea, Point4326, Point3035};
use kdtree::KdTree;
use kdtree::distance::squared_euclidean;
use rand::distributions::WeightedIndex;
use rand::prelude::*;

use crate::bounding_box::BoundingBox;


pub struct DensityClusters {
    dist: rand::distributions::weighted::WeightedIndex<u32>,
    points: Vec<Point4326>,
    weights: Vec<u32>,
    /// A k-d tree with indices into `points` and `weights`.
    kdtree: KdTree<f64, usize, [f64;2]>,
}

impl DensityClusters {
    pub fn from_csv<P: AsRef<Path>>(path: P, bounds: Option<BoundingBox>)
        -> anyhow::Result<Self>
    {
        println!("Read file {}", path.as_ref().to_string_lossy());
        let buf_reader = BufReader::new(File::open(path)?);
        let mut rdr = csv::ReaderBuilder::new()
            .has_headers(true)
            .from_reader(buf_reader);
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
        println!("  Done. ({} clusters)", points.len());
        let dist = WeightedIndex::new(&weights).unwrap();
        Ok(DensityClusters {
            dist,
            points,
            weights,
            kdtree,
        })
    }

    /// Return a random point from the distribution.
    pub fn sample_point(&self, rng: &mut ThreadRng) -> Point4326 {
        self.points[self.dist.sample(rng)]
    }

    /// Sample a point within a radius from a given point.
    /// Returns None if no destination point can be created because there aren't any or the sum of
    /// weights is zero.
    pub fn sample_point_within(&self, rng: &mut ThreadRng, from: Point4326, within_radius: f64)
        -> Option<Point4326>
    {
        let from = laea::forward(from);

        // Get point indices within given radius.
        // Errors only if the dimension is wrong or a coordinate is not finite.
        let indices = self.kdtree.within(
            &[from.coords.0, from.coords.1],
            // square distance
            within_radius.powi(2),
            &squared_euclidean,
        ).unwrap();

        match WeightedIndex::new(indices.iter().map(|p| self.weights[*p.1])) {
            Ok(distribution) => {
                let selected_index = indices[distribution.sample(rng)];
                Some(self.points[*selected_index.1])
            },
            Err(_) => None,
        }
    }
}
