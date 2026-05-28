#!/usr/bin/env python3
"""One-shot: deploy our Krexa test agent on-chain.

Reads KREXA_AGENT_SECRET_KEY from env, derives the pubkey, calls
POST /agents/deploy, signs the returned partial-signed tx with the
agent keypair, and submits to Solana mainnet-beta.

Idempotent: if the agent is already deployed, prints "exists" and exits 0.
"""
import base58
import base64
import json
import os
import sys
import urllib.request

from solders.keypair import Keypair
from solders.transaction import VersionedTransaction
from solders.message import to_bytes_versioned
from solders.signature import Signature

KREXA_API = "https://api.krexa.xyz/api/v1"
SOLANA_RPC = "https://api.mainnet-beta.solana.com"


def load_keypair() -> Keypair:
    secret_b58 = os.environ["KREXA_AGENT_SECRET_KEY"]
    secret_bytes = base58.b58decode(secret_b58)
    if len(secret_bytes) != 64:
        raise ValueError(f"expected 64-byte Solana keypair, got {len(secret_bytes)}")
    return Keypair.from_bytes(secret_bytes)


def http_json(method: str, url: str, body=None) -> dict:
    headers = {"Content-Type": "application/json"}
    data = json.dumps(body).encode() if body is not None else None
    req = urllib.request.Request(url, data=data, headers=headers, method=method)
    with urllib.request.urlopen(req, timeout=60) as resp:
        return json.loads(resp.read())


def main() -> int:
    kp = load_keypair()
    pubkey = str(kp.pubkey())
    print(f"Agent pubkey: {pubkey}")

    print("Calling /agents/deploy...")
    deploy = http_json("POST", f"{KREXA_API}/agents/deploy", {
        "agent": pubkey,
        "owner": pubkey,
        "name": "aomi-agent",
        "agentType": 0,
    })
    print(f"Status: {deploy.get('status')}")

    if deploy.get("status") == "exists":
        print("Agent already deployed.")
        return 0
    if deploy.get("status") != "ready":
        print(f"Unexpected deploy response: {deploy}", file=sys.stderr)
        return 1

    tx_b64 = deploy["transaction"]
    tx_bytes = base64.b64decode(tx_b64)
    print(f"Tx size: {len(tx_bytes)} bytes")

    # solders parses both legacy and versioned txs via VersionedTransaction
    tx = VersionedTransaction.from_bytes(tx_bytes)
    msg_bytes = to_bytes_versioned(tx.message)

    # The tx has slots for every signer; the oracle pre-filled its slot,
    # the agent's slot is a zeroed placeholder. We sign the message and
    # replace the agent's signature in-place.
    sigs = list(tx.signatures)
    account_keys = list(tx.message.account_keys)
    agent_pk = kp.pubkey()
    signer_indices = [i for i, k in enumerate(account_keys[: tx.message.header.num_required_signatures])]
    matched = False
    for i in signer_indices:
        if account_keys[i] == agent_pk:
            sigs[i] = kp.sign_message(msg_bytes)
            matched = True
            print(f"Signed at signer index {i} ({account_keys[i]})")
    if not matched:
        print("Agent pubkey not in required-signers — unexpected tx layout", file=sys.stderr)
        print(f"Signers: {[str(account_keys[i]) for i in signer_indices]}", file=sys.stderr)
        return 1

    signed_tx = VersionedTransaction.populate(tx.message, sigs)

    print("Submitting to Solana...")
    submit = http_json("POST", SOLANA_RPC, {
        "jsonrpc": "2.0",
        "id": 1,
        "method": "sendTransaction",
        "params": [
            base64.b64encode(bytes(signed_tx)).decode(),
            {"encoding": "base64", "skipPreflight": False, "preflightCommitment": "confirmed"},
        ],
    })
    if "error" in submit:
        print(f"sendTransaction error: {submit['error']}", file=sys.stderr)
        return 1
    sig = submit["result"]
    print(f"Submitted: {sig}")
    print(f"  https://solscan.io/tx/{sig}")
    print("Waiting up to 60s for confirmation...")

    import time
    for _ in range(30):
        status = http_json("POST", SOLANA_RPC, {
            "jsonrpc": "2.0",
            "id": 1,
            "method": "getSignatureStatuses",
            "params": [[sig], {"searchTransactionHistory": True}],
        })
        entry = status["result"]["value"][0]
        if entry is None:
            time.sleep(2)
            continue
        cs = entry.get("confirmationStatus")
        err = entry.get("err")
        if err:
            print(f"On-chain error: {err}", file=sys.stderr)
            return 1
        if cs in ("confirmed", "finalized"):
            print(f"Confirmed: {cs}")
            return 0
        print(f"  status: {cs}")
        time.sleep(2)
    print("Timed out waiting for confirmation; check Solscan.")
    return 1


if __name__ == "__main__":
    sys.exit(main())
