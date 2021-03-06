use geomatic::Point4326;
use serde::de::Deserialize;

use crate::route::{LatLon32, Route};

pub struct RoutingMachine {
    client: reqwest::blocking::Client,
}

impl RoutingMachine {
    pub fn new() -> Self {
        RoutingMachine {
            client: reqwest::blocking::Client::new(),
        }
    }

    pub fn test_connection(&self) -> anyhow::Result<()> {
        let resp = self.client.get(
            "http://127.0.0.1:5000/route/v1/nearest/0.0,0.0;0.0,0.0"
            )
            .send()?
            .text()?;

        let json_value: serde_json::Value = serde_json::from_str(&resp)?;

        match json_value.get("code").and_then(|v| v.as_str()) {
            Some("Ok") => Ok(()),
            _ => Err(anyhow::anyhow!("status code is not Ok")),
        }
    }

    pub fn find_route(&self, a: Point4326, b: Point4326) -> anyhow::Result<Route> {
        let resp = self.client.get(
            &format!("http://127.0.0.1:5000/route/v1/driving/{},{};{},{}", a.lon(), a.lat(), b.lon(), b.lat()))
            .query(&[("annotations", "nodes")])
            .send()?
            .text()?;

        let json_value: serde_json::Value = serde_json::from_str(&resp)?;
        let nodes_array = &json_value["routes"][0]["legs"][0]["annotation"]["nodes"];
        let node_ids = Vec::<_>::deserialize(nodes_array)?;
        let distance = json_value["routes"][0]["distance"]
            .as_f64()
            .ok_or_else(|| anyhow::anyhow!("Route has no 'distance' field"))?;

        let route = Route {
            start_coord: LatLon32::new(a.lat(), a.lon()),
            end_coord: LatLon32::new(b.lat(), b.lon()),
            node_ids,
            distance,
        };

        Ok(route)
    }
}
