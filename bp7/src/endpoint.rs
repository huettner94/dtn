// Copyright (C) 2023 Felix Huettner
//
// This file is part of DTRD.
//
// DTRD is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.
//
// DTRD is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU General Public License for more details.
//
// You should have received a copy of the GNU General Public License
// along with this program.  If not, see <https://www.gnu.org/licenses/>.

use std::fmt::Display;

use serde::{
    Deserialize, Serialize,
    de::{Error, Unexpected, Visitor},
    ser::SerializeSeq,
};
use serde_repr::{Deserialize_repr, Serialize_repr};

use crate::Validate;

#[derive(Debug, Serialize_repr, Deserialize_repr, PartialEq, Eq)]
#[repr(u64)]
enum EndpointType {
    Dtn = 1,
    Ipn = 2,
}

#[derive(Debug, PartialEq, Eq, PartialOrd, Ord, Clone, Hash)]
pub enum Endpoint {
    DTN(DTNEndpoint),
    IPN(IPNEndpoint),
}

impl Serialize for Endpoint {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let mut seq = serializer.serialize_seq(Some(2))?;
        match self {
            Endpoint::DTN(e) => {
                seq.serialize_element(&EndpointType::Dtn)?;
                seq.serialize_element(e)?;
            }
            Endpoint::IPN(e) => {
                seq.serialize_element(&EndpointType::Ipn)?;
                seq.serialize_element(e)?;
            }
        }
        seq.end()
    }
}

impl<'de> Deserialize<'de> for Endpoint {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        struct EndpointVisitor;
        impl<'de> Visitor<'de> for EndpointVisitor {
            type Value = Endpoint;

            fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
                formatter.write_str("endpoint")
            }

            fn visit_seq<A>(self, mut seq: A) -> Result<Self::Value, A::Error>
            where
                A: serde::de::SeqAccess<'de>,
            {
                let endpoint_type: EndpointType = seq
                    .next_element()?
                    .ok_or(Error::custom("Error for field 'endpoint_type'"))?;
                match endpoint_type {
                    EndpointType::Dtn => {
                        let dtn_endpoint: DTNEndpoint = seq
                            .next_element()?
                            .ok_or(Error::custom("Error for field 'dtn_endpoint'"))?;
                        Ok(Endpoint::DTN(dtn_endpoint))
                    }
                    EndpointType::Ipn => {
                        let ipn_endpoint: IPNEndpoint = seq
                            .next_element()?
                            .ok_or(Error::custom("Error for field 'ipn_endpoint'"))?;
                        Ok(Endpoint::IPN(ipn_endpoint))
                    }
                }
            }
        }
        deserializer.deserialize_seq(EndpointVisitor)
    }
}

impl Validate for Endpoint {
    fn validate(&self) -> bool {
        match self {
            Endpoint::DTN(e) => e.validate(),
            Endpoint::IPN(e) => e.validate(),
        }
    }
}

impl Display for Endpoint {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Endpoint::DTN(e) => e.fmt(f),
            Endpoint::IPN(e) => e.fmt(f),
        }
    }
}

impl Endpoint {
    pub fn new(uri: &str) -> Option<Self> {
        let (schema, content) = uri.split_once(':')?;
        match schema {
            "dtn" => Some(Endpoint::DTN(DTNEndpoint::from_str(content)?)),
            "ipn" => Some(Endpoint::IPN(IPNEndpoint::from_str(content)?)),
            _ => None,
        }
    }

    pub fn is_null_endpoint(&self) -> bool {
        match self {
            Endpoint::DTN(e) => e.is_null_endpoint(),
            Endpoint::IPN(_) => false,
        }
    }

    pub fn matches_node(&self, other: &Endpoint) -> bool {
        match self {
            Endpoint::DTN(s) => matches!(other, Endpoint::DTN(o) if s.matches_node(o)),
            Endpoint::IPN(s) => matches!(other, Endpoint::IPN(o) if s.matches_node(o)),
        }
    }

    pub fn get_node_endpoint(&self) -> Endpoint {
        match self {
            Endpoint::DTN(s) => Endpoint::DTN(s.get_node_endpoint()),
            Endpoint::IPN(s) => Endpoint::IPN(s.get_node_endpoint()),
        }
    }
}

#[derive(Debug, PartialEq, Eq, PartialOrd, Ord, Clone, Hash)]
pub struct DTNEndpoint {
    pub uri: String,
}

impl DTNEndpoint {
    fn from_str(uri: &str) -> Option<Self> {
        if !uri.starts_with("//") {
            return None;
        }
        Some(DTNEndpoint {
            uri: String::from(uri),
        })
    }

    fn is_null_endpoint(&self) -> bool {
        self.uri == "none"
    }

    pub fn node_name(&self) -> &str {
        self.uri[2..]
            .split('/')
            .next()
            .expect("There is always a first element")
    }

    pub fn matches_node(&self, other: &DTNEndpoint) -> bool {
        self.node_name() == other.node_name()
    }

    pub fn get_node_endpoint(&self) -> DTNEndpoint {
        DTNEndpoint::from_str(&("//".to_owned() + self.node_name())).unwrap()
    }
}

impl Serialize for DTNEndpoint {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        if self.is_null_endpoint() {
            serializer.serialize_u64(0)
        } else {
            serializer.serialize_str(&self.uri)
        }
    }
}

impl<'de> Deserialize<'de> for DTNEndpoint {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        struct DTNEndpointVisitor;
        impl<'de> Visitor<'de> for DTNEndpointVisitor {
            type Value = DTNEndpoint;

            fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
                formatter.write_str("DTN Endpoint")
            }

            fn visit_u64<E>(self, v: u64) -> Result<Self::Value, E>
            where
                E: Error,
            {
                if v == 0 {
                    return Ok(DTNEndpoint {
                        uri: String::from("none"),
                    });
                }
                Err(Error::invalid_value(
                    Unexpected::Unsigned(v),
                    &"DTN Endpoints may only have 0 as a value",
                ))
            }

            fn visit_borrowed_str<E>(self, v: &'de str) -> Result<Self::Value, E>
            where
                E: Error,
            {
                let endpoint = DTNEndpoint {
                    uri: String::from(v),
                };
                Ok(endpoint)
            }
        }
        deserializer.deserialize_any(DTNEndpointVisitor)
    }
}

impl Validate for DTNEndpoint {
    fn validate(&self) -> bool {
        if self.uri != "none" && !self.uri.starts_with("//") {
            return false;
        }
        true
    }
}

impl Display for DTNEndpoint {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_fmt(format_args!("dtn:{}", self.uri))
    }
}

#[derive(Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, Clone, Copy, Hash)]
pub struct IPNEndpoint {
    pub node: u64,
    pub service: u64,
}

impl Validate for IPNEndpoint {
    fn validate(&self) -> bool {
        true
    }
}

impl IPNEndpoint {
    fn from_str(uri: &str) -> Option<Self> {
        let (schema, hier) = uri.split_once(':')?;
        if schema != "ipn" {
            return None;
        }
        let (node, service) = hier.split_once('.')?;
        let node_id = node.parse().ok()?;
        let service_id = service.parse().ok()?;
        Some(IPNEndpoint {
            node: node_id,
            service: service_id,
        })
    }

    pub fn matches_node(&self, other: &IPNEndpoint) -> bool {
        self.node == other.node
    }

    pub fn get_node_endpoint(&self) -> IPNEndpoint {
        IPNEndpoint {
            node: self.node,
            service: 0,
        }
    }
}

impl Display for IPNEndpoint {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_fmt(format_args!("ipn:{}.{}", self.node, self.service))
    }
}
