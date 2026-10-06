//! The compiled contract's ABI and the campaign's named actors. Together
//! they turn the model's string-typed calls into exact calldata.

use std::collections::{BTreeMap, BTreeSet};
use std::str::FromStr;

use alloy_dyn_abi::{DynSolType, DynSolValue, JsonAbiExt, Specifier};
use alloy_json_abi::{Function, JsonAbi, StateMutability};
use alloy_primitives::{Address, B256, Bytes, FixedBytes, I256, U256, keccak256};
use serde_json::Value;

use crate::plan::FuzzActor;

pub(crate) struct ContractAbi(JsonAbi);

impl ContractAbi {
    pub fn parse(json: &str) -> Result<Self, String> {
        let abi: JsonAbi = serde_json::from_str(json)
            .map_err(|e| format!("abi must be a JSON array string: {e}"))?;
        if abi.functions().next().is_none() {
            return Err("ABI contains no callable functions".into());
        }
        Ok(Self(abi))
    }

    pub fn functions(&self) -> impl Iterator<Item = &Function> {
        self.0.functions()
    }

    pub fn inner(&self) -> &JsonAbi {
        &self.0
    }

    pub fn function(&self, signature: &str) -> Result<&Function, String> {
        self.0
            .functions()
            .find(|function| function.signature() == signature)
            .ok_or_else(|| format!("function `{signature}` is not in the ABI"))
    }

    pub fn read_only(function: &Function) -> bool {
        matches!(
            function.state_mutability,
            StateMutability::View | StateMutability::Pure
        )
    }

    /// Canonical signatures of every state-changing function.
    pub fn writable(&self) -> BTreeSet<String> {
        self.0
            .functions()
            .filter(|function| !Self::read_only(function))
            .map(Function::signature)
            .collect()
    }

    /// ABI-encode `signature(arguments)`. Each argument is a string; arrays
    /// and tuples are JSON text; `$name` resolves to that actor's address.
    pub fn encode(
        &self,
        actors: &Actors,
        signature: &str,
        arguments: &[String],
    ) -> Result<Bytes, String> {
        let function = self.function(signature)?;
        if function.inputs.len() != arguments.len() {
            return Err(format!(
                "`{signature}` takes {} arguments, got {}",
                function.inputs.len(),
                arguments.len()
            ));
        }
        let values = function
            .inputs
            .iter()
            .zip(arguments)
            .map(|(parameter, text)| {
                let ty = parameter.resolve().map_err(|e| e.to_string())?;
                let value = match ty {
                    DynSolType::Array(_) | DynSolType::FixedArray(..) | DynSolType::Tuple(_) => {
                        serde_json::from_str(text)
                            .map_err(|e| format!("array/tuple argument must be JSON text: {e}"))?
                    }
                    _ => Value::String(text.clone()),
                };
                actors.to_sol(&ty, &value)
            })
            .collect::<Result<Vec<_>, _>>()?;
        function
            .abi_encode_input(&values)
            .map(Bytes::from)
            .map_err(|e| e.to_string())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Actor {
    pub address: Address,
    pub balance: U256,
}

/// Named actors with addresses derived from the artifact, so the same plan
/// against the same contract always uses the same addresses.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Actors(BTreeMap<String, Actor>);

impl Actors {
    pub fn derive(artifact: B256, inputs: &[FuzzActor]) -> Result<Self, String> {
        let mut actors = BTreeMap::new();
        for input in inputs {
            input.validate()?;
            if actors.contains_key(&input.name) {
                return Err(format!("actor `{}` is declared twice", input.name));
            }
            let hash = keccak256([artifact.as_slice(), input.name.as_bytes()].concat());
            actors.insert(
                input.name.clone(),
                Actor {
                    address: Address::from_slice(&hash[12..]),
                    balance: U256::from_str(&input.balance)
                        .map_err(|_| format!("actor `{}` has an invalid balance", input.name))?,
                },
            );
        }
        Ok(Self(actors))
    }

    pub fn get(&self, name: &str) -> Result<&Actor, String> {
        self.0
            .get(name)
            .ok_or_else(|| format!("unknown actor `{name}`"))
    }

    pub fn iter(&self) -> impl Iterator<Item = (&String, &Actor)> {
        self.0.iter()
    }

    pub fn len(&self) -> usize {
        self.0.len()
    }

    fn to_sol(&self, ty: &DynSolType, value: &Value) -> Result<DynSolValue, String> {
        let resolved = match value.as_str().and_then(|text| text.strip_prefix('$')) {
            Some(name) => Value::String(format!("{:#x}", self.get(name)?.address)),
            None => value.clone(),
        };
        let text = || {
            resolved
                .as_str()
                .ok_or_else(|| format!("expected a string for {ty}"))
        };
        let list = |expected: Option<usize>| -> Result<Vec<&Value>, String> {
            let items = resolved
                .as_array()
                .ok_or_else(|| format!("expected a JSON array for {ty}"))?;
            match expected {
                Some(len) if items.len() != len => {
                    Err(format!("{ty} needs {len} items, got {}", items.len()))
                }
                _ => Ok(items.iter().collect()),
            }
        };
        match ty {
            DynSolType::Bool => resolved
                .as_bool()
                .or_else(|| text().ok()?.parse().ok())
                .map(DynSolValue::Bool)
                .ok_or_else(|| "expected a boolean".into()),
            DynSolType::Int(bits) => Ok(DynSolValue::Int(
                text()?.parse::<I256>().map_err(|e| e.to_string())?,
                *bits,
            )),
            DynSolType::Uint(bits) => Ok(DynSolValue::Uint(
                U256::from_str(text()?).map_err(|e| e.to_string())?,
                *bits,
            )),
            DynSolType::Address => Ok(DynSolValue::Address(
                text()?.parse().map_err(|e| format!("{e}"))?,
            )),
            DynSolType::FixedBytes(len) => {
                let bytes = Bytes::from_str(text()?).map_err(|e| e.to_string())?;
                if bytes.len() != *len {
                    return Err(format!("bytes{len} needs {len} bytes, got {}", bytes.len()));
                }
                let mut word = [0u8; 32];
                word[..*len].copy_from_slice(&bytes);
                Ok(DynSolValue::FixedBytes(FixedBytes::from(word), *len))
            }
            DynSolType::Bytes => Ok(DynSolValue::Bytes(
                Bytes::from_str(text()?)
                    .map_err(|e| e.to_string())?
                    .to_vec(),
            )),
            DynSolType::String => Ok(DynSolValue::String(text()?.to_owned())),
            DynSolType::Array(inner) => Ok(DynSolValue::Array(
                list(None)?
                    .into_iter()
                    .map(|item| self.to_sol(inner, item))
                    .collect::<Result<_, _>>()?,
            )),
            DynSolType::FixedArray(inner, len) => Ok(DynSolValue::FixedArray(
                list(Some(*len))?
                    .into_iter()
                    .map(|item| self.to_sol(inner, item))
                    .collect::<Result<_, _>>()?,
            )),
            DynSolType::Tuple(types) => Ok(DynSolValue::Tuple(
                types
                    .iter()
                    .zip(list(Some(types.len()))?)
                    .map(|(ty, item)| self.to_sol(ty, item))
                    .collect::<Result<_, _>>()?,
            )),
            other => Err(format!("unsupported ABI type {other}")),
        }
    }
}
