# Aomi SDK WebAssembly binding

This package loads any Aomi plugin built for `wasm32-unknown-unknown` with the
SDK's `dyn_aomi_app!` macro. It uses the same JSON lifecycle contract as the
native Rust `DynFnHandle`; the binding only translates C strings to and from
the module's exported linear memory.

```ts
import { readFile } from "node:fs/promises";
import { AomiWasmPlugin } from "@aomi-labs/sdk-wasm";

const plugin = await AomiWasmPlugin.instantiate(await readFile("my_app.wasm"));

try {
  console.log(plugin.manifest());
  const start = plugin.startTool(
    "greet",
    { name: "Ada" },
    {
      session_id: "demo",
      tool_name: "greet",
      call_id: "call-1",
      state_attributes: {},
      secrets: {},
    },
  );
  console.log(start);
} finally {
  plugin.dispose();
}
```

Build a plugin and run the native-versus-WASM parity test:

```sh
rustup target add wasm32-unknown-unknown
cd bindings/typescript
npm install
npm test
```

The reusable binding covers manifest discovery, synchronous tool results,
queued async polling, cancellation, errors, and instance cleanup. On raw
`wasm32-unknown-unknown`, an async Rust tool produces its queued events inline
during `startTool`; the JavaScript-visible start/poll envelope remains the same,
but there is no background thread inside the module.
