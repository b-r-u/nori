use serde::de::Deserialize;

use crate::route::Route;


pub struct RoutingMachine {
    client: reqwest::blocking::Client,
}

impl RoutingMachine {
    pub fn new() -> Self {
        RoutingMachine {
            client: reqwest::blocking::Client::new(),
        }
    }

    pub fn find_route(&self, a_lon: f64, a_lat: f64, b_lon: f64, b_lat: f64) -> Result<Route, Box<dyn std::error::Error>> {
        let resp = self.client.get(
            &format!("http://127.0.0.1:5000/route/v1/driving/{},{};{},{}", a_lon, a_lat, b_lon, b_lat))
            .query(&[("annotations", "nodes")])
            .send()?
            .text()?;

        let json_value: serde_json::Value = serde_json::from_str(&resp)?;

        let nodes_array = json_value
            .get("routes")
            .and_then(|x| x.get(0))
            .and_then(|x| x.get("legs"))
            .and_then(|x| x.get(0))
            .and_then(|x| x.get("annotation"))
            .and_then(|x| x.get("nodes"));

        let node_ids = match nodes_array {
            Some(arr) => Vec::<i64>::deserialize(arr).unwrap(),
            None => vec![],
        };

        let conv = |c: f64| {(c * 10e6) as i32};

        let route = Route {
            start_coord: (conv(a_lat), conv(a_lon)),
            end_coord: (conv(b_lat), conv(b_lon)),
            node_ids,
        };

        Ok(route)
    }
}
