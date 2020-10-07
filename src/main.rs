use std::fs::File;
use clap::{Arg, ArgGroup, App, AppSettings, SubCommand};


mod network;
mod route;
mod routing_machine;
mod sampling;

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
                 .required(true)
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
            .arg(Arg::with_name("uniform2d")
                 .long("uniform2d")
                 .value_name("sw.1 sw.2 ne.1 ne.2")
                 .help("Sets bounding box for a uniform sampling on the 2D plane. Input values are
                       the two coordinate pairs for the south-west and the north-east corner of the
                       bounding box")
                 .takes_value(true)
                 .number_of_values(4)
                 .validator(is_number::<f64>)
             )
            .arg(Arg::with_name("weighted")
                 .long("weighted")
                 .value_name("FILE.csv")
                 .help("sample from a list of weighted points from the given CSV file.")
                 .takes_value(true)
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
        let geojson_path = matches.value_of("geojson").unwrap();

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
            let aabb: Vec<_> = matches.values_of("uniform2d").unwrap()
                .map(|s| s.parse::<f64>().unwrap()).collect();
            assert_eq!(aabb.len(), 4);
            let mut uni_sample = sampling::Uniform2D::new(aabb[0], aabb[1], aabb[2], aabb[3]);
            sample(&mut uni_sample, number_of_samples, &mut machine, &mut writer, &mut net)?;
        } else if matches.is_present("weighted") {
            let csv_path = matches.value_of("weighted").unwrap();
            //let mut sampl = sampling::Weighted::from_csv(csv_path)?;
            //sample(&mut sampl, number_of_samples, &mut machine, &mut writer, &mut net)?;
        }

        writer.finish()?;
        net.write_to_geojson(geojson_path)?;
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
    for _ in 0..number_of_samples {
        let a = sampl.gen_point();
        let b = sampl.gen_point();

        println!("rand {:?} {:?}", a, b);
        let res = machine.find_route(a.0, a.1, b.0, b.1)?;
        let res = writer.write_route(res)?;
        net.bump_edges(&res.node_ids);
    }
    Ok(())
}

