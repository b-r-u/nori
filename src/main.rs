use std::fs::File;
use clap::{Arg, ArgGroup, App, AppSettings, SubCommand};
use geomatic::Point4326;


mod bounding_box;
mod network;
mod route;
mod routing_machine;
mod sampling;

use bounding_box::BoundingBox;
use network::Network;
use route::RouteCollectionWriter;
use routing_machine::RoutingMachine;
use sampling::Sampling;


fn main() -> Result<(), Box<dyn std::error::Error>> {

    let matches = App::new("nori - naive aggregated traffic estimation")
        .version("0.1")
        .author("Johannes Hofmann <mail@b-r-u.org>")
        .about("Estimate average daily traffic on a road network by sampling a distribution of shortest paths.")
        .setting(AppSettings::ArgRequiredElseHelp)
        .subcommand(SubCommand::with_name("sample")
            .about("Create pairs of points and store the shortest paths between them.")
            .arg(Arg::with_name("osrm")
                 .long("osrm")
                 .value_name("FILE")
                 .help("Sets an input *.osrm file")
                 .takes_value(true)
                 .required(true)
            )
            .arg(Arg::with_name("routes")
                 .long("routes")
                 .value_name("FILE")
                 .help("Sets the output file to store the routes")
                 .takes_value(true)
                 .required(true)
             )
            .arg(Arg::with_name("geojson")
                 .long("geojson")
                 .value_name("FILE")
                 .help("Sets the output GeoJSON file to store the road network with traffic counts")
                 .takes_value(true)
             )
            .arg(Arg::with_name("png")
                 .long("png")
                 .value_name("FILE")
                 .help("Sets the output PNG file to store a rendering of the road network with traffic counts")
                 .takes_value(true)
             )
            .arg(Arg::with_name("number")
                 .long("number")
                 .short("n")
                 .value_name("INT")
                 .help("Sets the number of samples")
                 .takes_value(true)
                 .required(true)
                 .validator(is_number::<u32>)
             )
            .arg(Arg::with_name("bounds")
                 .long("bounds")
                 .value_name("sw.lat sw.lon ne.lat ne.lon")
                 .help("Sets the bounding box. Input values are the two coordinate pairs for the
                       south-west and the north-east corner of the bounding box")
                 .takes_value(true)
                 .number_of_values(4)
                 .validator(is_number::<f64>)
             )
            .arg(Arg::with_name("max_dist")
                 .long("max-dist")
                 .value_name("METERS")
                 .help("Sets the maximum distance between source and destination points in
                       meters.")
                 .takes_value(true)
                 .validator(is_number::<f64>)
             )
            .arg(Arg::with_name("uniform2d")
                 .long("uniform2d")
                 .help("Sample the 2D plane uniformly.")
                 .requires_all(&["bounds", "max_dist"])
             )
            .arg(Arg::with_name("weighted")
                 .long("weighted")
                 .value_name("FILE.csv")
                 .help("sample from a list of weighted points from the given CSV file.")
                 .takes_value(true)
                 .requires_all(&["max_dist"])
             )
            .group(ArgGroup::with_name("sampling")
                 .args(&["uniform2d", "weighted"])
                 .required(true))
        )
        .subcommand(SubCommand::with_name("routes")
            .about("Read *.routes files.")
            .arg(Arg::with_name("input")
                 .long("input")
                 .value_name("FILE")
                 .help("Sets an input *.routes file")
                 .takes_value(true)
                 .required(true)
            )
        )
        .get_matches();


    if let Some(matches) = matches.subcommand_matches("sample") {
        let number_of_samples = matches.value_of("number").unwrap().parse::<u32>().unwrap();
        let osrm_path = matches.value_of("osrm").unwrap();
        let routes_path = matches.value_of("routes").unwrap();

        let bounds = if matches.is_present("bounds") {
            let aabb: Vec<_> = matches.values_of("bounds").unwrap()
                .map(|s| s.parse::<f64>().unwrap()).collect();
            assert_eq!(aabb.len(), 4);
            Some(BoundingBox::new(
                Point4326::new(aabb[0], aabb[1]),
                Point4326::new(aabb[2], aabb[3]))
            )
        } else {
            None
        };

        let mut machine = RoutingMachine::new();
        machine.test_connection()?;

        println!("Read *.osrm file {:?}", osrm_path);
        let mut net = Network::from_path(osrm_path)?;
        let mut writer = RouteCollectionWriter::new(
            routes_path,
            osrm_path,
            "sample",
        )?;

        if matches.is_present("uniform2d") {
            let max_dist: f64 = matches.value_of("max_dist").unwrap().parse::<f64>()?;
            let mut uni_sample = sampling::Uniform2D::new(bounds.unwrap(), max_dist);
            sample(&mut uni_sample, number_of_samples, &mut machine, &mut writer, &mut net)?;
        } else if matches.is_present("weighted") {
            let max_dist: f64 = matches.value_of("max_dist").unwrap().parse::<f64>()?;
            let csv_path = matches.value_of("weighted").unwrap();
            let mut sampl = sampling::Weighted::from_csv(csv_path, bounds, max_dist)?;
            sample(&mut sampl, number_of_samples, &mut machine, &mut writer, &mut net)?;
        }

        writer.finish()?;

        if let Some(geojson_path) = matches.value_of("geojson") {
            net.write_to_geojson(geojson_path)?;
        }

        if let Some(png_path) = matches.value_of("png") {
            if let Some(bounds) = bounds {
                net.render_png(png_path, bounds, 2048, 2048);
            } else {
                net.render_png(png_path, net.get_bounds(), 2048, 2048);
            }
        }
    } else if let Some(matches) = matches.subcommand_matches("routes") {
        let routes_path = matches.value_of("input").unwrap();
        let reader = route::RouteCollectionReader::new(&routes_path)?;
        println!("{:?}", reader.header());

        for (i, route) in reader.enumerate() {
            println!("Route #{}: {} nodes", i + 1, route?.node_ids.len());
        }
    }

    Ok(())
}


fn is_number<T: std::str::FromStr>(s: String) -> Result<(), String> {
    match s.parse::<T>() {
        Ok(_) => Ok(()),
        Err(_) => Err(format!("need a number")),
    }
}


fn sample<S: Sampling>(
    sampl: &mut S,
    number_of_samples: u32,
    machine: &mut RoutingMachine,
    writer: &mut RouteCollectionWriter<File>,
    net: &mut Network,
) -> Result<(), Box<dyn std::error::Error>>
{
    for i in 0..number_of_samples {
        let a = sampl.gen_source();
        let b = sampl.gen_destination(a);

        println!("{:.2}%, {}: {} {}", (100.0 * (i + 1) as f64) / (number_of_samples as f64), i + 1, a, b);
        let res = machine.find_route(a, b)?;
        let res = writer.write_route(res)?;
        net.bump_edges(&res.node_ids);
    }
    Ok(())
}

