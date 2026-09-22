# Credential demo

This app exercises required and optional user-owned secret slots without
calling a real provider or performing financial operations. Its main tool,
`credential_demo_validate`, sends the current user's values to the loopback
mock and returns a safe profile label. It never returns either credential.
`credential_demo_redaction_probe` deliberately places the harmless required
demo token in a nested value and an object key so a local host can verify that
tool results are redacted before model delivery.

Start the mock:

```sh
python3 sdk/examples/credential-demo/mock_server.py
```

The mock accepts these harmless test tokens:

| `DEMO_API_TOKEN` | Returned profile |
| --- | --- |
| `demo-token-a` | `demo-account-a` |
| `demo-token-b` | `demo-account-b` |
| `demo-token-replacement` | `demo-account-a-rotated` |

`DEMO_ACCOUNT_TAG` is optional. When present, the response reports only
`optional_credential_present: true`.

Build the plugin from the repository root:

```sh
cargo build -p credential-demo
```

The Linux artifact is `target/debug/libcredential_demo.so`; macOS produces
`target/debug/libcredential_demo.dylib`.

For a local backend that already has an application row, stage a source-bound
bundle with its positive application ID and matching release tag:

```sh
python3 sdk/examples/credential-demo/stage_bundle.py \
  --plugins-root /path/to/runtime/plugins \
  --application-id 123 \
  --library /path/to/libcredential_demo.so \
  --release-tag local-credential-demo
```

The backend must use Aomi SDK 5.1.1 and the application row must use the same
release tag before it loads this bundle.
