use geomatic::Point4326;
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

    pub fn test_connection(&self) -> Result<(), Box<dyn std::error::Error>> {
        let resp = self.client.get(
            "http://127.0.0.1:5000/route/v1/nearest/0.0,0.0;0.0,0.0"
            )
            .send()?
            .text()?;

        let json_value: serde_json::Value = serde_json::from_str(&resp)?;

        match json_value.get("code").and_then(|v| v.as_str()) {
            Some("Ok") => Ok(()),
            _ => Err(Box::new(std::io::Error::new(std::io::ErrorKind::Other, "status code is not Ok"))),
        }
    }

    pub fn find_route(&self, a: Point4326, b: Point4326) -> Result<Route, Box<dyn std::error::Error>> {
        let resp = self.client.get(
            &format!("http://127.0.0.1:5000/route/v1/driving/{},{};{},{}", a.lon(), a.lat(), b.lon(), b.lat()))
            .query(&[("annotations", "nodes")])
            .send()?
            .text()?;

        let json_value: serde_json::Value = serde_json::from_str(&resp)?;
        let nodes_array = &json_value["routes"][0]["legs"][0]["annotation"]["nodes"];
        let node_ids = Vec::<i64>::deserialize(nodes_array)?;

        let conv = |c: f64| {(c * 10e6) as i32};

        let route = Route {
            start_coord: (conv(a.lat()), conv(a.lon())),
            end_coord: (conv(b.lat()), conv(b.lon())),
            node_ids,
        };

        Ok(route)
    }
}
