//! Outcome builders: `data`, `prepared` and `sign`, mapped to the WIT `outcome`.

use serde::Serialize;
use serde_json::Value;

use crate::bindings::aomi::package::types::{
    Attachment, DataReply, Erc191Domain, EvmPersonal, EvmTypedData, FailureRule, FollowUp,
    ImageType, NextCall, Outcome as WitOutcome, PreparedReply, PreparedTx, SignReply, SignRequest,
    StageKind, SvmMessage, SvmMessageDomain,
};
use crate::{Error, Image, Result, StageTx, err};

/// The most continuation state a `sign` outcome may carry, in bytes of JSON.
const STATE_CAP: usize = 16 * 1024;

/// What a tool returns. A builder misuse or a serialization failure is kept
/// and surfaces as the tool's error when the call returns.
#[derive(Debug)]
pub struct Outcome(Result<WitOutcome>);

impl Outcome {
    /// JSON for the model (or, for a direct tool, the channel).
    pub fn data(value: impl Serialize) -> Self {
        Self(json(&value).map(|value| {
            WitOutcome::Data(DataReply {
                value,
                attachments: Vec::new(),
            })
        }))
    }

    /// Attaches an image to a `data` outcome; the host allows two.
    pub fn image(self, image: Image) -> Self {
        self.edit(|o| match o {
            WitOutcome::Data(d) => {
                d.attachments.push(image);
                Ok(())
            }
            _ => Err(misuse("image", "data")),
        })
    }

    /// Transactions for the core staging lanes; add them with [`Outcome::tx`].
    pub fn prepared(summary: impl Serialize) -> Self {
        Self(json(&summary).map(|summary| {
            WitOutcome::Prepared(PreparedReply {
                summary,
                txs: Vec::new(),
                failure_rules: Vec::new(),
            })
        }))
    }

    pub fn tx(self, tx: StageTx) -> Self {
        self.edit(|o| match o {
            WitOutcome::Prepared(p) => {
                p.txs.push(tx);
                Ok(())
            }
            _ => Err(misuse("tx", "prepared")),
        })
    }

    /// Names the `continuation` tool to call about the last added transaction.
    pub fn follow_up(self, tool: &str, note: &str, data: &impl Serialize) -> Self {
        let data = json(data);
        self.edit(|o| match o {
            WitOutcome::Prepared(p) => {
                let tx = p
                    .txs
                    .last_mut()
                    .ok_or_else(|| err("outcome_invalid", "follow_up() needs a tx() before it"))?;
                tx.follow_up = Some(FollowUp {
                    tool: tool.into(),
                    note: note.into(),
                    data: data?,
                });
                Ok(())
            }
            _ => Err(misuse("follow_up", "prepared")),
        })
    }

    pub fn failure_rule(self, any_of: &[&str], classification: &str, advice: &str) -> Self {
        self.edit(|o| match o {
            WitOutcome::Prepared(p) => {
                p.failure_rules.push(FailureRule {
                    any_of: any_of.iter().map(|s| s.to_string()).collect(),
                    classification: classification.into(),
                    advice: advice.into(),
                });
                Ok(())
            }
            _ => Err(misuse("failure_rule", "prepared")),
        })
    }

    /// One signature request; [`Outcome::then`] is required.
    pub fn sign(summary: impl Serialize, request: SignRequest) -> Self {
        Self(json(&summary).map(|summary| {
            WitOutcome::Sign(SignReply {
                summary,
                request,
                then: NextCall {
                    tool: String::new(),
                    note: String::new(),
                    state: String::new(),
                    expires_at: None,
                },
            })
        }))
    }

    /// The `continuation` tool to call once signed, with at most 16 KiB of state.
    pub fn then(self, tool: &str, note: &str, state: &impl Serialize) -> Self {
        let state = json(state);
        self.edit(|o| match o {
            WitOutcome::Sign(s) => {
                let state = state?;
                if state.len() > STATE_CAP {
                    return Err(err(
                        "state_too_large",
                        format!(
                            "continuation state is {} bytes; at most {STATE_CAP}",
                            state.len()
                        ),
                    ));
                }
                s.then.tool = tool.into();
                s.then.note = note.into();
                s.then.state = state;
                Ok(())
            }
            _ => Err(misuse("then", "sign")),
        })
    }

    /// Unix seconds; the host caps it at 30 minutes after the outcome.
    pub fn expires_at(self, unix_secs: u64) -> Self {
        self.edit(|o| match o {
            WitOutcome::Sign(s) => {
                s.then.expires_at = Some(unix_secs);
                Ok(())
            }
            _ => Err(misuse("expires_at", "sign")),
        })
    }

    /// The WIT outcome, or the first error a builder kept.
    pub fn wit(self) -> Result<WitOutcome> {
        let o = self.0?;
        if let WitOutcome::Sign(s) = &o
            && s.then.tool.is_empty()
        {
            return Err(err("outcome_invalid", "a sign outcome needs then()"));
        }
        Ok(o)
    }

    fn edit(self, f: impl FnOnce(&mut WitOutcome) -> Result<()>) -> Self {
        Self(self.0.and_then(|mut o| f(&mut o).map(|()| o)))
    }
}

fn json(value: &impl Serialize) -> Result<String> {
    Ok(serde_json::to_string(value)?)
}

fn misuse(builder: &str, kind: &str) -> Error {
    err(
        "outcome_invalid",
        format!("{builder}() needs a {kind} outcome"),
    )
}

impl Attachment {
    pub fn png(bytes: Vec<u8>) -> Self {
        Self {
            kind: ImageType::Png,
            bytes,
            caption: None,
        }
    }

    pub fn jpeg(bytes: Vec<u8>) -> Self {
        Self {
            kind: ImageType::Jpeg,
            bytes,
            caption: None,
        }
    }

    /// At most 1024 characters.
    pub fn caption(mut self, caption: impl Into<String>) -> Self {
        self.caption = Some(caption.into());
        self
    }
}

impl PreparedTx {
    /// `evm_stage_tx` with its `StageTxParameters`.
    pub fn evm(args: Value) -> Self {
        Self::stage(StageKind::EvmStageTx, args)
    }

    /// `svm_stage_ix` with its `SvmStageIxParameters`.
    pub fn svm_ix(args: Value) -> Self {
        Self::stage(StageKind::SvmStageIx, args)
    }

    /// `svm_stage_tx` with its `SvmStageTxParameters`.
    pub fn svm_tx(args: Value) -> Self {
        Self::stage(StageKind::SvmStageTx, args)
    }

    /// The HTTP record this transaction rests on: [`crate::http::Response::record`].
    pub fn evidence(mut self, record: u32) -> Self {
        self.evidence = Some(record);
        self
    }

    fn stage(stage: StageKind, args: Value) -> Self {
        Self {
            stage,
            args: args.to_string(),
            evidence: None,
            follow_up: None,
        }
    }
}

impl SignRequest {
    /// An EIP-712 typed-data object.
    pub fn typed_data(typed_data: Value, description: impl Into<String>) -> Self {
        Self::EvmTypedData(EvmTypedData {
            typed_data: typed_data.to_string(),
            description: description.into(),
        })
    }

    /// An EIP-191 personal message.
    pub fn personal(
        message: impl Into<String>,
        description: impl Into<String>,
        domain: Erc191Domain,
    ) -> Self {
        Self::EvmPersonal(EvmPersonal {
            message: message.into(),
            description: description.into(),
            domain,
            request_kind: None,
        })
    }

    /// Raw Solana message bytes.
    pub fn svm_message(
        message: Vec<u8>,
        description: impl Into<String>,
        domain: SvmMessageDomain,
    ) -> Self {
        Self::SvmMessage(SvmMessage {
            message,
            description: description.into(),
            domain,
            kind: None,
        })
    }
}
