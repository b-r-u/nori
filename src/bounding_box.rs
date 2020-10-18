use geomatic::{laea, Point3035, Point4326};


#[derive(Copy, Clone, Debug)]
pub struct BoundingBox {
    pub sw: Point4326,
    pub ne: Point4326,
}

#[derive(Copy, Clone, Debug)]
pub struct BoundingBox3035 {
    pub sw: Point3035,
    pub ne: Point3035,
}

impl BoundingBox {
    pub fn new(sw: Point4326, ne: Point4326) -> Self {
        BoundingBox { sw, ne }
    }

    ///FIXME This is a naive implementation that does not handle the 180th meridian.
    pub fn is_inside(&self, point: Point4326) -> bool {
        point.coords.0 >= self.sw.coords.0 &&
        point.coords.1 >= self.sw.coords.1 &&
        point.coords.0 <= self.ne.coords.0 &&
        point.coords.1 <= self.ne.coords.1
    }

    pub fn get_3035_bounds(&self) -> BoundingBox3035 {
        let sw: Point3035 = laea::forward(self.sw);
        let ne: Point3035 = laea::forward(self.ne);
        let se: Point3035 = laea::forward(Point4326::new(self.sw.lat(), self.ne.lon()));
        let nw: Point3035 = laea::forward(Point4326::new(self.ne.lat(), self.sw.lon()));

        let bound_sw = Point3035::new(
            sw.coords.0.min(nw.coords.0),
            sw.coords.1.min(se.coords.1),
        );
        let bound_ne = Point3035::new(
            se.coords.0.max(ne.coords.0),
            nw.coords.1.max(ne.coords.1),
        );

        BoundingBox3035 {
            sw: bound_sw,
            ne: bound_ne,
        }
    }
}
