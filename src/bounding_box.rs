use geomatic::Point4326;


#[derive(Copy, Clone, Debug)]
pub struct BoundingBox {
    pub sw: Point4326,
    pub ne: Point4326,
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
}
