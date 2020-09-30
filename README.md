NORI - naive aggregated road traffic estimation
===============================================

Estimate average daily traffic on a road network by sampling a distribution of shortest paths.


## Instructions

* Download, compile, install OSRM

```bash
git clone https://github.com/Project-OSRM/osrm-backend/
cd osrm-backend
mkdir build
cd build
cmake ..
cmake --build .
sudo cmake --build . --target install
```

* Construct routing graph for area of interest

```bash
# Download OSM extract
wget http://download.geofabrik.de/europe/germany/berlin-latest.osm.pbf

# Build routing graph
osrm-extract -p /usr/local/share/osrm/profiles/car.lua berlin-latest.osm.pbf
osrm-partition berlin-latest.osrm
osrm-customize berlin-latest.osrm
```

* Start OSRM backend server

```bash
osrm-routed --algorithm mld berlin-latest.osrm
```

* Compile this project

```bash
cargo build --release
```

* Run this project

```bash
cargo run --release -- sample -n 1000 --osrm ~/Downloads/berlin-latest.osrm --geojson output/berlin.geojson --routes output/berlin.routes --uniform2d 13.2392 52.4422 13.5125 52.5738
```


## Ideas

* log-normal distribution of trip lengths
* start/endpoints weighted by population-density, POI-density
* local conversion factor for ground truth


## TODO

* simplify geometry (merge lanes)
* Fix interesting OSRM fail (brandenburg-latest.osrm):

```
Error: reqwest::Error { kind: Request, url: "http://127.0.0.1:5000/route/v1/driving/13.272295023439796,52.49581830313898;13.383933517069835,52.54885321672839?annotations=nodes", source: hyper::Error(IncompleteMessage) }
```
