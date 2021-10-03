use serde::{
    de::{Error, Unexpected, Visitor},
    ser::SerializeSeq,
    Deserialize, Serialize,
};
use serde_repr::{Deserialize_repr, Serialize_repr};

use crate::bp7::Validate;

#[derive(Debug, Serialize_repr, Deserialize_repr, PartialEq, Eq)]
#[repr(u64)]
enum EndpointType {
    DTN = 1,
    IPN = 2,
}

#[derive(Debug, PartialEq, Eq, PartialOrd, Ord, Clone)]
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
                seq.serialize_element(&EndpointType::DTN)?;
                seq.serialize_element(e)?;
            }
            Endpoint::IPN(e) => {
                seq.serialize_element(&EndpointType::IPN)?;
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
                    EndpointType::DTN => {
                        let dtn_endpoint: DTNEndpoint = seq
                            .next_element()?
                            .ok_or(Error::custom("Error for field 'dtn_endpoint'"))?;
                        return Ok(Endpoint::DTN(dtn_endpoint));
                    }
                    EndpointType::IPN => {
                        let ipn_endpoint: IPNEndpoint = seq
                            .next_element()?
                            .ok_or(Error::custom("Error for field 'ipn_endpoint'"))?;
                        return Ok(Endpoint::IPN(ipn_endpoint));
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

impl Endpoint {
    pub fn new(uri: &str) -> Option<Self> {
        let (schema, content) = uri.split_once(":")?;
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
}

#[derive(Debug, PartialEq, Eq, PartialOrd, Ord, Clone)]
pub struct DTNEndpoint {
    pub uri: String,
}

impl DTNEndpoint {
    fn from_str(uri: &str) -> Option<Self> {
        if !uri.starts_with("//") {
            return None;
        }
        return Some(DTNEndpoint {
            uri: String::from(uri),
        });
    }

    fn is_null_endpoint(&self) -> bool {
        return self.uri == "none";
    }

    pub fn node_name(&self) -> &str {
        self.uri[2..]
            .split('/')
            .next()
            .expect("There is always a first element")
    }

    pub fn matches_node(&self, other: &DTNEndpoint) -> bool {
        return self.node_name() == other.node_name();
    }
}

impl Serialize for DTNEndpoint {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        if self.is_null_endpoint() {
            return serializer.serialize_u64(0);
        } else {
            return serializer.serialize_str(&self.uri);
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
                return Err(Error::invalid_value(
                    Unexpected::Unsigned(v),
                    &"DTN Endpoints may only have 0 as a value",
                ));
            }

            fn visit_borrowed_str<E>(self, v: &'de str) -> Result<Self::Value, E>
            where
                E: Error,
            {
                let endpoint = DTNEndpoint {
                    uri: String::from(v),
                };
                return Ok(endpoint);
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
        return true;
    }
}

#[derive(Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, Clone, Copy)]
pub struct IPNEndpoint {
    pub node: u64,
    pub serivce: u64,
}

impl Validate for IPNEndpoint {
    fn validate(&self) -> bool {
        return true;
    }
}

impl IPNEndpoint {
    fn from_str(uri: &str) -> Option<Self> {
        let (schema, hier) = uri.split_once(":")?;
        if schema != "ipn" {
            return None;
        }
        let (node, service) = hier.split_once(".")?;
        let node_id = node.parse().ok()?;
        let service_id = service.parse().ok()?;
        return Some(IPNEndpoint {
            node: node_id,
            serivce: service_id,
        });
    }

    pub fn matches_node(&self, other: &IPNEndpoint) -> bool {
        return self.node == other.node;
    }
}
