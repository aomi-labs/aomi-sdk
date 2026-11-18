//! Stateless EVM simulation over the host's `simulation.run`: a fresh fork at
//! latest, this thread's staged transactions replayed first, then the calls in
//! order and the reads against the post-state. Nothing persists or is sent.
//! The types serialize with `0x` hex bytes, so a tool can take them as
//! arguments or return a report as data.

use serde::{Deserialize, Serialize};

use crate::Result;
use crate::bindings::aomi::package::simulation as host;

/// Runs one simulation; the host allows four per call.
pub async fn run(request: &Request) -> Result<Report> {
    Ok(host::run(&request.wit())?.into())
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Request {
    pub chain_id: u64,
    /// Sender of every call: an EVM wallet in scope.
    pub from: String,
    /// Staged transaction ids replayed first, as `chain::View::Staged`.
    #[serde(default)]
    pub prefix: Vec<u32>,
    /// At most 16, state carried forward.
    #[serde(default)]
    pub calls: Vec<Call>,
    /// At most 16, after the last call.
    #[serde(default)]
    pub reads: Vec<Read>,
}

impl Request {
    pub fn new(chain_id: u64, from: impl Into<String>) -> Self {
        Self {
            chain_id,
            from: from.into(),
            prefix: Vec::new(),
            calls: Vec::new(),
            reads: Vec::new(),
        }
    }

    fn wit(&self) -> host::SimRequest {
        host::SimRequest {
            chain_id: self.chain_id,
            from: self.from.clone(),
            prefix: self.prefix.clone(),
            calls: self.calls.iter().map(Call::wit).collect(),
            reads: self
                .reads
                .iter()
                .map(|r| match r {
                    Read::Call(c) => host::SimRead::Call(c.wit()),
                    Read::Balance(a) => host::SimRead::Balance(a.clone()),
                })
                .collect(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Call {
    pub to: String,
    #[serde(with = "hex")]
    pub data: Vec<u8>,
    /// Wei as a decimal string; empty means zero.
    #[serde(default)]
    pub value: String,
}

impl Call {
    /// A call sending no value.
    pub fn new(to: impl Into<String>, data: impl Into<Vec<u8>>) -> Self {
        Self {
            to: to.into(),
            data: data.into(),
            value: String::new(),
        }
    }

    /// Wei as a decimal string.
    pub fn value(mut self, wei: impl Into<String>) -> Self {
        self.value = wei.into();
        self
    }

    fn wit(&self) -> host::SimCall {
        host::SimCall {
            to: self.to.clone(),
            data: self.data.clone(),
            value: self.value.clone(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Read {
    /// `eth_call` against the post-state.
    Call(Call),
    /// Native balance of an address in the post-state.
    Balance(String),
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Report {
    pub block: u64,
    pub block_hash: String,
    /// One per call, in order.
    pub calls: Vec<CallResult>,
    /// One per read: return data, or the 32-byte big-endian balance.
    pub reads: Vec<ReadResult>,
}

impl From<host::SimResult> for Report {
    fn from(r: host::SimResult) -> Self {
        Self {
            block: r.block,
            block_hash: r.block_hash,
            calls: r.calls.into_iter().map(CallResult::from).collect(),
            reads: r
                .reads
                .into_iter()
                .map(|r| match r {
                    Ok(bytes) => ReadResult::Ok(bytes),
                    Err(e) => ReadResult::Err(e),
                })
                .collect(),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Status {
    Succeeded,
    Reverted,
    Halted,
    Failed,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CallResult {
    pub status: Status,
    #[serde(with = "hex")]
    pub return_data: Vec<u8>,
    pub gas_used: u64,
    pub logs: Vec<Log>,
}

impl From<host::SimOutcome> for CallResult {
    fn from(o: host::SimOutcome) -> Self {
        Self {
            status: match o.status {
                host::SimStatus::Succeeded => Status::Succeeded,
                host::SimStatus::Reverted => Status::Reverted,
                host::SimStatus::Halted => Status::Halted,
                host::SimStatus::Failed => Status::Failed,
            },
            return_data: o.return_data,
            gas_used: o.gas_used,
            logs: o
                .logs
                .into_iter()
                .map(|l| Log {
                    address: l.address,
                    topics: l.topics,
                    data: l.data,
                })
                .collect(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Log {
    pub address: String,
    #[serde(with = "hex::list")]
    pub topics: Vec<Vec<u8>>,
    #[serde(with = "hex")]
    pub data: Vec<u8>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReadResult {
    Ok(#[serde(with = "hex")] Vec<u8>),
    /// Why the read failed, as the host reported it.
    Err(String),
}

/// `0x`-prefixed lowercase hex for byte fields.
mod hex {
    use serde::de::Error as _;
    use serde::{Deserialize, Deserializer, Serializer};

    pub fn encode(bytes: &[u8]) -> String {
        let mut s = String::with_capacity(2 + bytes.len() * 2);
        s.push_str("0x");
        for b in bytes {
            s.push_str(&format!("{b:02x}"));
        }
        s
    }

    pub fn decode(s: &str) -> Result<Vec<u8>, String> {
        let digits = s.strip_prefix("0x").unwrap_or(s);
        if !digits.len().is_multiple_of(2) {
            return Err(format!("odd-length hex `{s}`"));
        }
        (0..digits.len())
            .step_by(2)
            .map(|i| {
                u8::from_str_radix(digits.get(i..i + 2).unwrap_or("x"), 16)
                    .map_err(|_| format!("invalid hex `{s}`"))
            })
            .collect()
    }

    pub fn serialize<S: Serializer>(bytes: &[u8], s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(&encode(bytes))
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(d: D) -> Result<Vec<u8>, D::Error> {
        decode(&String::deserialize(d)?).map_err(D::Error::custom)
    }

    pub mod list {
        use super::*;

        pub fn serialize<S: Serializer>(items: &[Vec<u8>], s: S) -> Result<S::Ok, S::Error> {
            s.collect_seq(items.iter().map(|b| encode(b)))
        }

        pub fn deserialize<'de, D: Deserializer<'de>>(d: D) -> Result<Vec<Vec<u8>>, D::Error> {
            Vec::<String>::deserialize(d)?
                .iter()
                .map(|s| decode(s).map_err(D::Error::custom))
                .collect()
        }
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    const USDC: &str = "0x833589fcd6edb6e08f4c7c32d4f71b54bda02913";
    const OWNER: &str = "0x00000000219ab540356cbb839cbe05303d7705fa";

    #[test]
    fn a_request_reads_from_json_and_lowers_to_the_wit_record() {
        let request: Request = serde_json::from_value(json!({
            "chain_id": 8453,
            "from": OWNER,
            "prefix": [7],
            "calls": [{ "to": USDC, "data": "0x095ea7b3", "value": "" }],
            "reads": [
                { "call": { "to": USDC, "data": "0x70a08231" } },
                { "balance": OWNER }
            ]
        }))
        .unwrap();
        let built = Request {
            prefix: vec![7],
            calls: vec![Call::new(USDC, [0x09, 0x5e, 0xa7, 0xb3])],
            reads: vec![
                Read::Call(Call::new(USDC, [0x70, 0xa0, 0x82, 0x31])),
                Read::Balance(OWNER.into()),
            ],
            ..Request::new(8453, OWNER)
        };
        assert_eq!(request, built);
        assert_eq!(
            serde_json::to_value(&built).unwrap()["calls"][0]["data"],
            "0x095ea7b3"
        );

        let wit = built.wit();
        assert_eq!((wit.chain_id, wit.prefix.as_slice()), (8453, &[7][..]));
        assert_eq!(wit.calls[0].data, [0x09, 0x5e, 0xa7, 0xb3]);
        assert!(matches!(&wit.reads[1], host::SimRead::Balance(a) if a == OWNER));
        assert_eq!(Call::new(USDC, []).value("1000").wit().value, "1000");
    }

    #[test]
    fn a_result_lifts_to_a_report_that_serializes_as_hex() {
        let result = host::SimResult {
            block: 23_000_000,
            block_hash: "0xabc".into(),
            calls: vec![host::SimOutcome {
                status: host::SimStatus::Reverted,
                return_data: vec![0x08, 0xc3, 0x79, 0xa0],
                gas_used: 21_512,
                logs: vec![host::SimLog {
                    address: USDC.into(),
                    topics: vec![vec![0xdd; 32]],
                    data: vec![1],
                }],
            }],
            reads: vec![Ok(vec![0; 32]), Err("execution reverted".into())],
        };
        let report = Report::from(result);
        let value = serde_json::to_value(&report).unwrap();
        assert_eq!(
            value["calls"][0],
            json!({
                "status": "reverted",
                "return_data": "0x08c379a0",
                "gas_used": 21512,
                "logs": [{
                    "address": USDC,
                    "topics": [format!("0x{}", "dd".repeat(32))],
                    "data": "0x01"
                }]
            })
        );
        assert_eq!(value["reads"][1], json!({ "err": "execution reverted" }));
        assert_eq!(serde_json::from_value::<Report>(value).unwrap(), report);
    }

    #[test]
    fn bad_hex_is_refused() {
        for data in ["0x0", "0xzz", "0x€€"] {
            let call = json!({ "to": USDC, "data": data });
            assert!(serde_json::from_value::<Call>(call).is_err(), "{data}");
        }
    }
}
