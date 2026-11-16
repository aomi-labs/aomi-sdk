//! Unsigned Solana transactions, decoded in the guest: legacy and v0 messages,
//! with v0 address lookup tables read through `chain::svm(..).account` and
//! resolved into plain instructions. Every length and index is checked; a
//! malformed transaction is `svm_tx_invalid`, a bad lookup table
//! `svm_table_invalid`. An [`Instruction`] serializes in the shape
//! `svm_stage_ix` takes (`program_id`, `accounts`, `data_base64`).

use std::fmt;
use std::str::FromStr;

use base64::Engine as _;
use base64::engine::general_purpose::STANDARD;
use serde::{Serialize, Serializer};
use serde_json::Value;

use crate::chain::Svm;
use crate::{Error, Result, err};

/// The most bytes a transaction may have: one packet.
const PACKET_SIZE: usize = 1232;
/// The address lookup table program, which owns every table account.
const TABLE_PROGRAM: &str = "AddressLookupTab1e1111111111111111111111111";
/// Bytes of a table account before its addresses.
const TABLE_META_SIZE: usize = 56;
/// The table account discriminator of an initialized table.
const TABLE_INITIALIZED: u32 = 1;

/// A 32-byte account address, base58 as text.
#[derive(Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Pubkey(pub [u8; 32]);

impl FromStr for Pubkey {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self> {
        let mut key = [0; 32];
        match bs58::decode(s).onto(&mut key) {
            Ok(32) => Ok(Self(key)),
            _ => Err(err(
                "svm_pubkey_invalid",
                format!("`{s}` is not a 32-byte address"),
            )),
        }
    }
}

impl fmt::Display for Pubkey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&bs58::encode(self.0).into_string())
    }
}

impl fmt::Debug for Pubkey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(self, f)
    }
}

impl Serialize for Pubkey {
    fn serialize<S: Serializer>(&self, s: S) -> std::result::Result<S::Ok, S::Error> {
        s.collect_str(self)
    }
}

/// A transaction as a venue returns it: its signatures and its message.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Transaction {
    /// One per required signer; all zero while unsigned.
    pub signatures: Vec<[u8; 64]>,
    pub message: Message,
}

impl Transaction {
    pub fn from_base64(text: &str) -> Result<Self> {
        let bytes = STANDARD
            .decode(text.trim())
            .map_err(|e| invalid(format!("base64: {e}")))?;
        Self::decode(&bytes)
    }

    /// Decodes the wire bytes; trailing bytes and a signature count other
    /// than the header's are refused.
    pub fn decode(bytes: &[u8]) -> Result<Self> {
        if bytes.len() > PACKET_SIZE {
            return Err(invalid(format!(
                "{} bytes, more than a {PACKET_SIZE}-byte packet",
                bytes.len()
            )));
        }
        let mut r = Reader { bytes, at: 0 };
        let count = r.len()?;
        let signatures = (0..count)
            .map(|_| r.array())
            .collect::<Result<Vec<[u8; 64]>>>()?;
        let message = Message::read(&mut r)?;
        r.end()?;
        if signatures.len() != usize::from(message.header.required_signatures) {
            return Err(invalid(format!(
                "{} signatures for {} required signers",
                signatures.len(),
                message.header.required_signatures
            )));
        }
        Ok(Self {
            signatures,
            message,
        })
    }

    /// True when no signature is filled in.
    pub fn is_unsigned(&self) -> bool {
        self.signatures.iter().all(|s| s.iter().all(|b| *b == 0))
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Version {
    Legacy,
    V0,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Header {
    /// Signers, the fee payer first; the first keys of the message.
    pub required_signatures: u8,
    /// The last this many signers are read-only.
    pub readonly_signed: u8,
    /// The last this many static keys are read-only.
    pub readonly_unsigned: u8,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Message {
    pub version: Version,
    pub header: Header,
    /// The static account keys, signers first.
    pub keys: Vec<Pubkey>,
    pub blockhash: [u8; 32],
    pub instructions: Vec<CompiledInstruction>,
    /// v0 only: the tables whose entries extend the account list.
    pub lookups: Vec<Lookup>,
}

/// An instruction by account index: static keys, then every table's writable
/// entries, then every table's read-only entries.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CompiledInstruction {
    pub program: u8,
    pub accounts: Vec<u8>,
    pub data: Vec<u8>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Lookup {
    pub table: Pubkey,
    pub writable: Vec<u8>,
    pub readonly: Vec<u8>,
}

impl Message {
    fn read(r: &mut Reader) -> Result<Self> {
        let first = r.byte()?;
        let version = match first {
            0x80 => Version::V0,
            b if b & 0x80 != 0 => {
                return Err(invalid(format!("message version {}", b & 0x7f)));
            }
            _ => Version::Legacy,
        };
        let required_signatures = match version {
            Version::V0 => r.byte()?,
            Version::Legacy => first,
        };
        let header = Header {
            required_signatures,
            readonly_signed: r.byte()?,
            readonly_unsigned: r.byte()?,
        };
        let count = r.len()?;
        let keys = (0..count)
            .map(|_| r.array().map(Pubkey))
            .collect::<Result<Vec<_>>>()?;
        let blockhash = r.array()?;
        let count = r.len()?;
        let instructions = (0..count)
            .map(|_| {
                Ok(CompiledInstruction {
                    program: r.byte()?,
                    accounts: r.vec()?,
                    data: r.vec()?,
                })
            })
            .collect::<Result<Vec<_>>>()?;
        let lookups = match version {
            Version::Legacy => Vec::new(),
            Version::V0 => {
                let count = r.len()?;
                (0..count)
                    .map(|_| {
                        Ok(Lookup {
                            table: Pubkey(r.array()?),
                            writable: r.vec()?,
                            readonly: r.vec()?,
                        })
                    })
                    .collect::<Result<Vec<_>>>()?
            }
        };
        let message = Self {
            version,
            header,
            keys,
            blockhash,
            instructions,
            lookups,
        };
        message.check()?;
        Ok(message)
    }

    fn check(&self) -> Result<()> {
        let h = self.header;
        if h.required_signatures == 0 || h.readonly_signed >= h.required_signatures {
            return Err(invalid("the fee payer must be a writable signer"));
        }
        if usize::from(h.required_signatures) + usize::from(h.readonly_unsigned) > self.keys.len() {
            return Err(invalid("the header counts more keys than the message has"));
        }
        let loaded: usize = self
            .lookups
            .iter()
            .map(|l| l.writable.len() + l.readonly.len())
            .sum();
        if self.keys.len() + loaded > 256 {
            return Err(invalid("more than 256 accounts"));
        }
        Ok(())
    }

    pub fn fee_payer(&self) -> &Pubkey {
        // `check` guarantees at least one signer key.
        &self.keys[0]
    }

    /// The instructions with every account resolved; `tables` are the
    /// message's lookup tables in its own order.
    pub fn instructions(&self, tables: &[Table]) -> Result<Vec<Instruction>> {
        if tables.len() != self.lookups.len()
            || tables
                .iter()
                .zip(&self.lookups)
                .any(|(t, l)| t.key != l.table)
        {
            return Err(err(
                "svm_table_invalid",
                "the tables do not match the message's lookups",
            ));
        }
        let h = self.header;
        let signers = usize::from(h.required_signatures);
        let writable_signers = signers - usize::from(h.readonly_signed);
        let writable_unsigned = self.keys.len() - usize::from(h.readonly_unsigned);
        let mut accounts: Vec<AccountMeta> = self
            .keys
            .iter()
            .enumerate()
            .map(|(i, key)| AccountMeta {
                pubkey: *key,
                is_signer: i < signers,
                is_writable: if i < signers {
                    i < writable_signers
                } else {
                    i < writable_unsigned
                },
            })
            .collect();
        for writable in [true, false] {
            for (table, lookup) in tables.iter().zip(&self.lookups) {
                let indexes = if writable {
                    &lookup.writable
                } else {
                    &lookup.readonly
                };
                for index in indexes {
                    let pubkey = *table.addresses.get(usize::from(*index)).ok_or_else(|| {
                        err(
                            "svm_table_invalid",
                            format!("table {} has no entry {index}", table.key),
                        )
                    })?;
                    accounts.push(AccountMeta {
                        pubkey,
                        is_signer: false,
                        is_writable: writable,
                    });
                }
            }
        }
        let at = |index: u8| {
            accounts
                .get(usize::from(index))
                .copied()
                .ok_or_else(|| invalid(format!("account index {index} is out of range")))
        };
        self.instructions
            .iter()
            .map(|ix| {
                // A program is always a static key, never a table entry.
                let program_id = *self.keys.get(usize::from(ix.program)).ok_or_else(|| {
                    invalid(format!("program index {} is not static", ix.program))
                })?;
                Ok(Instruction {
                    program_id,
                    accounts: ix.accounts.iter().map(|i| at(*i)).collect::<Result<_>>()?,
                    data: ix.data.clone(),
                })
            })
            .collect()
    }

    /// Reads the lookup tables through `svm` and resolves the instructions.
    pub async fn resolve(&self, svm: &Svm) -> Result<Vec<Instruction>> {
        let mut tables = Vec::with_capacity(self.lookups.len());
        for lookup in &self.lookups {
            tables.push(Table::fetch(svm, lookup.table).await?);
        }
        self.instructions(&tables)
    }
}

/// An address lookup table's entries.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Table {
    pub key: Pubkey,
    pub addresses: Vec<Pubkey>,
}

impl Table {
    /// Decodes a table account's data.
    pub fn decode(key: Pubkey, data: &[u8]) -> Result<Self> {
        let bad = |why: String| err("svm_table_invalid", format!("table {key}: {why}"));
        let (meta, entries) = data
            .split_at_checked(TABLE_META_SIZE)
            .ok_or_else(|| bad(format!("{} bytes, shorter than its header", data.len())))?;
        let kind = u32::from_le_bytes([meta[0], meta[1], meta[2], meta[3]]);
        if kind != TABLE_INITIALIZED {
            return Err(bad(format!(
                "account kind {kind}, not an initialized table"
            )));
        }
        let (addresses, rest) = entries.as_chunks::<32>();
        if !rest.is_empty() {
            return Err(bad(format!("{} entry bytes", entries.len())));
        }
        Ok(Self {
            key,
            addresses: addresses.iter().map(|a| Pubkey(*a)).collect(),
        })
    }

    /// Reads the table account with `getAccountInfo` and decodes it.
    pub async fn fetch(svm: &Svm, key: Pubkey) -> Result<Self> {
        let bad = |why: &str| err("svm_table_invalid", format!("table {key}: {why}"));
        let reply = svm.account(&key.to_string()).await?;
        let account = reply
            .get("value")
            .filter(|v| !v.is_null())
            .ok_or_else(|| bad("no such account"))?;
        if account.get("owner").and_then(Value::as_str) != Some(TABLE_PROGRAM) {
            return Err(bad("not owned by the lookup table program"));
        }
        let data = account
            .pointer("/data/0")
            .and_then(Value::as_str)
            .and_then(|d| STANDARD.decode(d).ok())
            .ok_or_else(|| bad("account data is not base64"))?;
        Self::decode(key, &data)
    }
}

/// An instruction with its accounts resolved.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct Instruction {
    pub program_id: Pubkey,
    pub accounts: Vec<AccountMeta>,
    #[serde(rename = "data_base64", serialize_with = "base64")]
    pub data: Vec<u8>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
pub struct AccountMeta {
    pub pubkey: Pubkey,
    pub is_signer: bool,
    pub is_writable: bool,
}

fn base64<S: Serializer>(data: &[u8], s: S) -> std::result::Result<S::Ok, S::Error> {
    s.serialize_str(&STANDARD.encode(data))
}

fn invalid(why: impl Into<String>) -> Error {
    err("svm_tx_invalid", why)
}

/// A cursor over wire bytes; every read is bounds-checked.
struct Reader<'a> {
    bytes: &'a [u8],
    at: usize,
}

impl Reader<'_> {
    fn take(&mut self, n: usize) -> Result<&[u8]> {
        let out = self
            .bytes
            .get(self.at..self.at + n)
            .ok_or_else(|| invalid(format!("truncated at byte {}", self.at)))?;
        self.at += n;
        Ok(out)
    }

    fn byte(&mut self) -> Result<u8> {
        Ok(self.take(1)?[0])
    }

    fn array<const N: usize>(&mut self) -> Result<[u8; N]> {
        Ok(self.take(N)?.try_into().expect("take returns N bytes"))
    }

    /// A compact-u16 length: 1 to 3 bytes, 7 bits each, canonical only.
    fn len(&mut self) -> Result<usize> {
        let mut value = 0usize;
        for i in 0..3 {
            let b = self.byte()?;
            value |= usize::from(b & 0x7f) << (7 * i);
            if b & 0x80 == 0 {
                if (i > 0 && b == 0) || value > usize::from(u16::MAX) {
                    return Err(invalid(format!("bad compact length at byte {}", self.at)));
                }
                return Ok(value);
            }
        }
        Err(invalid(format!("bad compact length at byte {}", self.at)))
    }

    fn vec(&mut self) -> Result<Vec<u8>> {
        let n = self.len()?;
        Ok(self.take(n)?.to_vec())
    }

    fn end(&self) -> Result<()> {
        match self.bytes.len() - self.at {
            0 => Ok(()),
            n => Err(invalid(format!("{n} trailing bytes"))),
        }
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;
    use crate::chain::fake::{self, ready};
    use crate::chain::svm;

    /// A v0 swap as a venue returns it, unsigned: compute budget, then a
    /// Raydium AMM swap whose token program, pool, vaults and market come from
    /// one lookup table (writable entries 1, 2, 3; read-only 0, 4).
    const V0_TX: &str = "AQAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAACAAQACBH6MCIdgv94d3c8ywX8gm4JC7lKq8TH6zYjQ6ixtCwbyYDLCAMevWidab0pML5QPtMTjJSlAU8Rc/OiZrIaTZecDBkZv5SEXMv/srbpyw5vnvIzlu8X3EmssQ5s6QAAAAEvZScQ2AsM/IHeQ7RajUkyhuZdc8SGiqQz/7H34torNAAECAwQFBgcICQoLDA0ODxAREhMUFRYXGBkaGxwdHh8CAgAFAkANAwADBwcEBQYIAQARCUBCDwAAAAAAMBsPAAAAAAABGY8fTDpFImPUE7LNF+vLwaDliHNk5iYaEqgXkuoWWj4DAQIDAgAE";
    /// The lookup table account's data: header, then five addresses.
    const ALT_DATA: &str = "AQAAAP//////////gLLmDgAAAAAAAX6MCIdgv94d3c8ywX8gm4JC7lKq8TH6zYjQ6ixtCwbyAAAG3fbh12Whk9mbdspWIi2TVkPFodQzb3StYwoq/v8AqT1uRy5npG6mtL0Lq539NeK0xy8dbVnC6rlclCVzrSLxuHDhLdN5iRVh0un6jyZDGDTrc28vJPwqKk3/H9XcpN/yy7m3YO3bGFcGMDBjrTPXtXKW6gLU4DNeMc6vpMxC3Q0HUagoLaYTBf4pnDe5mOWEcdsRNQNzEPi+EEWmCvbu";
    /// A legacy SOL transfer of 5000 lamports.
    const LEGACY_TX: &str = "AQAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAABAAEDfowIh2C/3h3dzzLBfyCbgkLuUqrxMfrNiNDqLG0LBvJgMsIAx69aJ1pvSkwvlA+0xOMlKUBTxFz86JmshpNl5wAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAECAwQFBgcICQoLDA0ODxAREhMUFRYXGBkaGxwdHh8BAgIAAQwCAAAAiBMAAAAAAAA=";

    const PAYER: &str = "9WzDXwBbmkg8ZTbNMqUxvQRAyrZzDsGYdLVL9zYtAWWM";
    const USER: &str = "7UX2i7SucgLMQcfZ75s3VXmZZY4YRUyJN9X1RgfMoDUi";
    const ALT: &str = "2immgwYNHBbyVQKVGCEkgWpi53bLwWNRMB5G2nbgYV17";

    fn key(s: &str) -> Pubkey {
        s.parse().unwrap()
    }

    fn meta(pubkey: &str, is_signer: bool, is_writable: bool) -> AccountMeta {
        AccountMeta {
            pubkey: key(pubkey),
            is_signer,
            is_writable,
        }
    }

    fn table_account(owner: &str, data: &str) -> Value {
        json!({
            "context": { "slot": 250_000_100 },
            "value": {
                "data": [data, "base64"],
                "executable": false,
                "lamports": 11_000_000,
                "owner": owner,
                "rentEpoch": 18_446_744_073_709_551_615u64,
                "space": 216
            }
        })
    }

    fn expect_table(reply: Value) {
        fake::svm(
            "mainnet-beta",
            "getAccountInfo",
            json!([ALT, { "encoding": "base64" }]),
            reply,
        );
    }

    #[test]
    fn a_v0_swap_resolves_its_lookup_table() {
        let tx = Transaction::from_base64(V0_TX).unwrap();
        assert!(tx.is_unsigned());
        let message = &tx.message;
        assert_eq!(message.version, Version::V0);
        assert_eq!(message.fee_payer(), &key(PAYER));
        assert_eq!(message.lookups[0].table, key(ALT));

        expect_table(table_account(TABLE_PROGRAM, ALT_DATA));
        let ixs = ready(message.resolve(&svm("mainnet-beta"))).unwrap();
        fake::done();

        assert_eq!(ixs.len(), 2);
        assert_eq!(
            ixs[0].program_id,
            key("ComputeBudget111111111111111111111111111111")
        );
        assert_eq!(ixs[0].data, [2, 0x40, 0x0d, 0x03, 0x00]);
        assert!(ixs[0].accounts.is_empty());

        let swap = &ixs[1];
        assert_eq!(
            swap.program_id,
            key("675kPX9MHTjS2zt1qfr1NYHuzeLXfQM9H24wFSUt1Mp8")
        );
        assert_eq!(
            swap.accounts,
            [
                meta("TokenkegQfeYyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA", false, false),
                meta("58oQChx4yWmvKdwLLZzBi4ChoCc2fqCUWBkwMihLYQo2", false, true),
                meta("DQyrAcCrDXQ7NeoqGgDCZwBvWDcYmFCjSb9JtteuvPpz", false, true),
                meta("HLmqeL62xR1QoZ1HKKbXRrdN1p3phKpxRMb2VVopvBBz", false, true),
                meta("srmqPvymJeFKQ4zGQed1GFppgkRHL9kaELCbyksJtPX", false, false),
                meta(USER, false, true),
                meta(PAYER, true, true),
            ]
        );
        let staged = serde_json::to_value(swap).unwrap();
        assert_eq!(
            staged["program_id"],
            "675kPX9MHTjS2zt1qfr1NYHuzeLXfQM9H24wFSUt1Mp8"
        );
        assert_eq!(
            staged["accounts"][6],
            json!({ "pubkey": PAYER, "is_signer": true, "is_writable": true })
        );
        assert_eq!(staged["data_base64"], "CUBCDwAAAAAAMBsPAAAAAAA=");
    }

    #[test]
    fn a_legacy_transfer_decodes_without_tables() {
        let tx = Transaction::from_base64(LEGACY_TX).unwrap();
        assert_eq!(tx.message.version, Version::Legacy);
        let ixs = ready(tx.message.resolve(&svm("mainnet-beta"))).unwrap();
        assert_eq!(ixs[0].program_id, key("11111111111111111111111111111111"));
        assert_eq!(
            ixs[0].accounts,
            [meta(PAYER, true, true), meta(USER, false, true)]
        );
        assert_eq!(ixs[0].data[..4], [2, 0, 0, 0]);
    }

    #[test]
    fn malformed_transactions_are_refused() {
        let bytes = STANDARD.decode(V0_TX).unwrap();
        let code = |b: &[u8]| Transaction::decode(b).unwrap_err().code;
        // Truncated anywhere, or with a trailing byte.
        for cut in [0, 1, 64, 66, 100, bytes.len() - 1] {
            assert_eq!(code(&bytes[..cut]), "svm_tx_invalid", "cut at {cut}");
        }
        assert_eq!(code(&[&bytes[..], &[0]].concat()), "svm_tx_invalid");
        // Two signatures for one required signer.
        let two = [&[2][..], &[0; 64], &bytes[1..]].concat();
        assert_eq!(code(&two), "svm_tx_invalid");
        // Message version 1.
        let mut v1 = bytes.clone();
        v1[65] = 0x81;
        assert_eq!(code(&v1), "svm_tx_invalid");
        // A non-canonical compact length (0x81 0x00 for 1).
        let padded = [&[0x81, 0x00][..], &bytes[1..]].concat();
        assert_eq!(code(&padded), "svm_tx_invalid");
        // More than a packet.
        assert_eq!(code(&vec![0; PACKET_SIZE + 1]), "svm_tx_invalid");
    }

    #[test]
    fn bad_lookup_tables_are_refused() {
        let message = Transaction::from_base64(V0_TX).unwrap().message;
        let data = STANDARD.decode(ALT_DATA).unwrap();
        let code = |data: &[u8]| Table::decode(key(ALT), data).unwrap_err().code;
        assert_eq!(code(&data[..TABLE_META_SIZE - 1]), "svm_table_invalid");
        assert_eq!(code(&data[..data.len() - 1]), "svm_table_invalid");
        let mut uninitialized = data.clone();
        uninitialized[0] = 0;
        assert_eq!(code(&uninitialized), "svm_table_invalid");

        // Only three entries: readonly index 4 is out of range.
        let short = Table::decode(key(ALT), &data[..TABLE_META_SIZE + 3 * 32]).unwrap();
        assert_eq!(
            message.instructions(&[short]).unwrap_err().code,
            "svm_table_invalid"
        );
        // A table other than the lookup's.
        let other = Table {
            key: key(PAYER),
            addresses: Vec::new(),
        };
        assert_eq!(
            message.instructions(&[other]).unwrap_err().code,
            "svm_table_invalid"
        );

        expect_table(table_account(PAYER, ALT_DATA));
        expect_table(json!({ "context": { "slot": 1 }, "value": null }));
        for _ in 0..2 {
            let e = ready(message.resolve(&svm("mainnet-beta"))).unwrap_err();
            assert_eq!(e.code, "svm_table_invalid");
        }
        fake::done();
    }

    #[test]
    fn pubkeys_round_trip_base58() {
        assert_eq!(key(ALT).to_string(), ALT);
        assert_eq!(
            "abc".parse::<Pubkey>().unwrap_err().code,
            "svm_pubkey_invalid"
        );
    }
}
