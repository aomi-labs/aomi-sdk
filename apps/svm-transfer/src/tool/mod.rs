//! Tool layer for svm-transfer.
//!
//! Two submodules — one per lane — plus shared helpers that mirror
//! Marinade's `tool/mod.rs`: wallet address resolution, base58 validation,
//! and the ix-shape struct that gets serialized into the
//! `SvmStageIx({instructions: [...]})` envelope.

pub(crate) mod transfer_ix;
pub(crate) mod transfer_tx;

use aomi_sdk::*;

/// Resolve the connected SVM wallet address. Both lanes require a
/// connected wallet — Lane 1 needs the payer for the staged ixs, Lane 2
/// needs it as the fee payer when building the tx blob.
pub(crate) fn require_svm_wallet(ctx: &DynToolCallCtx) -> Result<String, String> {
    ctx.attribute_string(&["domain", "svm", "address"])
        .ok_or_else(|| {
            "[svm-transfer] no SVM wallet connected — set SOLANA_KEYPAIR (or run \
             `aomi secret add SOLANA_KEYPAIR=…`) and re-open the session"
                .to_string()
        })
}

/// Validate a base58 Solana address. The host's stage paths re-validate,
/// but failing fast here gives the LLM a clearer error than the host's
/// parse-pubkey path would.
pub(crate) fn validate_base58_address(addr: &str) -> Result<(), String> {
    if addr.is_empty() {
        return Err("address is empty".to_string());
    }
    // Solana base58 addresses are 32-44 chars (32 bytes encoded). We
    // don't decode here — solana_sdk::pubkey does the canonical check
    // in transfer_tx; for Lane 1 we just gate on the rough shape.
    if addr.len() < 32 || addr.len() > 44 {
        return Err(format!(
            "address `{addr}` is not a base58 pubkey (32-44 chars expected)"
        ));
    }
    if !addr
        .chars()
        .all(|c| "123456789ABCDEFGHJKLMNPQRSTUVWXYZabcdefghijkmnopqrstuvwxyz".contains(c))
    {
        return Err(format!(
            "address `{addr}` contains characters outside the base58 alphabet"
        ));
    }
    Ok(())
}

/// The ix-shape that `SvmStageIx` accepts (host-side
/// `AssembledSvmIx`-shaped wire). Mirrors Marinade's `MarinadeIx`.
#[derive(Debug, Clone, serde::Serialize)]
pub(crate) struct TransferIx {
    pub program_id: String,
    pub accounts: Vec<TransferAcct>,
    pub data_base64: String,
    pub description: String,
}

#[derive(Debug, Clone, serde::Serialize)]
pub(crate) struct TransferAcct {
    pub pubkey: String,
    pub is_signer: bool,
    pub is_writable: bool,
}

/// System Program ID — base58 of 32 zero bytes.
pub(crate) const SYSTEM_PROGRAM_ID: &str = "11111111111111111111111111111111";

/// Build System Program transfer instruction data:
/// 4 bytes LE discriminator (=2 for Transfer) + 8 bytes LE lamports.
pub(crate) fn system_transfer_data(amount_lamports: u64) -> Vec<u8> {
    let mut buf = Vec::with_capacity(12);
    buf.extend_from_slice(&2u32.to_le_bytes());
    buf.extend_from_slice(&amount_lamports.to_le_bytes());
    buf
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn system_transfer_data_layout() {
        // Known good — 0.001 SOL = 1_000_000 lamports.
        let bytes = system_transfer_data(1_000_000);
        assert_eq!(bytes.len(), 12);
        // First 4 bytes = 2 (Transfer discriminator), LE.
        assert_eq!(&bytes[0..4], &[0x02, 0x00, 0x00, 0x00]);
        // Next 8 bytes = 1_000_000 LE = 0x40 0x42 0x0F 0x00 0x00 0x00 0x00 0x00.
        assert_eq!(
            &bytes[4..12],
            &[0x40, 0x42, 0x0F, 0x00, 0x00, 0x00, 0x00, 0x00]
        );
    }

    #[test]
    fn validate_base58_rejects_short_and_non_base58() {
        assert!(validate_base58_address("").is_err());
        assert!(validate_base58_address("tooshort").is_err());
        // Contains '0' which is not in base58 alphabet.
        assert!(validate_base58_address("1111111111111111111111111111111110").is_err());
        // 32 byte all-1 = base58 of 32 zero bytes = System Program ID.
        assert!(validate_base58_address(SYSTEM_PROGRAM_ID).is_ok());
    }
}
