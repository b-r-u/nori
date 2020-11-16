//! A simple interface for writing GeoJSON feature collections

use std::fmt::Debug;
use std::fs::File;
use std::io::{BufWriter, Write};
use std::path::Path;

use geomatic::Point4326;


/// Write feature collections
pub struct GeoJsonWriter<W: Write> {
    writer: BufWriter<W>,
    is_first_feature: bool,
    finished: bool,
}


impl GeoJsonWriter<File> {
    pub fn from_path<P: AsRef<Path>>(path: P)
        -> anyhow::Result<GeoJsonWriter<File>>
    {
        let file = File::create(path)?;
        Self::new(file)
    }
}

impl<W: Write> GeoJsonWriter<W> {
    pub fn new(writer: W)
        -> anyhow::Result<GeoJsonWriter<W>>
    {
        let mut writer = BufWriter::new(writer);
        writer.write(b"{\"type\": \"FeatureCollection\", \"features\": [")?;
        Ok(GeoJsonWriter {
            writer,
            is_first_feature: true,
            finished: false,
        })
    }

    pub fn add_line_string(&mut self, coords: &[Point4326]) -> anyhow::Result<FeatureWriter<W>> {
        if self.is_first_feature {
            self.is_first_feature = false;
        } else {
            write!(self.writer, ",")?;
        }

        write!(
            self.writer,
            "\n{{\"type\": \"Feature\", \
               \"geometry\": {{\
                 \"type\": \"LineString\", \
                 \"coordinates\": [\
             ",
        )?;

        for (i, point) in coords.iter().enumerate() {
            if i > 0 {
                write!(self.writer, ",")?;
            }
            write!(
                self.writer,
                "[{lon:.6}, {lat:.6}]",
                lon = point.lon(),
                lat = point.lat(),
            )?;
        }

        write!(self.writer, "]}}, \"properties\": {{")?;

        Ok(FeatureWriter {
            gjwriter: self,
            is_first: true,
            finished: false,
        })
    }

    pub fn add_point(&mut self, coord: Point4326) -> anyhow::Result<FeatureWriter<W>> {
        if self.is_first_feature {
            self.is_first_feature = false;
        } else {
            write!(self.writer, ",")?;
        }
        write!(
            self.writer,
            "\n{{\"type\": \"Feature\", \
               \"geometry\": {{\
                 \"type\": \"Point\", \
                 \"coordinates\": [{lon:.6}, {lat:.6}]}}, \
               \"properties\": {{\
             ",
            lon = coord.lon(),
            lat = coord.lat(),
        )?;
        Ok(FeatureWriter {
            gjwriter: self,
            is_first: true,
            finished: false,
        })
    }

    pub fn finish(mut self) -> anyhow::Result<()> {
        self.mut_finish()
    }

    /// A private method that does not move self so Drop can call it.
    fn mut_finish(&mut self) -> anyhow::Result<()> {
        if !self.finished {
            self.writer.write_all(b"\n]}")?;
            self.writer.flush()?;
            self.finished = true;
        }
        Ok(())
    }
}

impl<W: Write> Drop for GeoJsonWriter<W> {
    fn drop(&mut self) {
        // drop can't return errors :(
        let _ = self.mut_finish();
    }
}

pub struct FeatureWriter<'a, W: Write> {
    gjwriter: &'a mut GeoJsonWriter<W>,
    is_first: bool,
    finished: bool,
}

impl<'a, W: Write> FeatureWriter<'a, W> {
    pub fn add_property<D: Debug>(&mut self, key: &str, value: D) -> anyhow::Result<()> {
        if self.is_first {
            self.is_first = false;
        } else {
            write!(self.gjwriter.writer, ",")?;
        }
        write!(
            self.gjwriter.writer,
            "\"{}\": {:?}",
            key,
            value,
        )?;
        Ok(())
    }

    pub fn finish(mut self) -> anyhow::Result<()> {
        self.mut_finish()
    }

    /// A private method that does not move self so Drop can call it.
    fn mut_finish(&mut self) -> anyhow::Result<()> {
        if !self.finished {
            self.gjwriter.writer.write_all(b"}}")?;
            self.finished = true;
        }
        Ok(())
    }
}

impl<'a, W: Write> Drop for FeatureWriter<'a, W> {
    fn drop(&mut self) {
        // drop can't return errors :(
        let _ = self.mut_finish();
    }
}
