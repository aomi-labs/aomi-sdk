import assert from "node:assert/strict";
import { execFileSync } from "node:child_process";
import { readFile } from "node:fs/promises";
import { fileURLToPath } from "node:url";
import test from "node:test";

import {
  AomiWasmPlugin,
  type AsyncExecPoll,
  type DynToolCallContext,
  type JsonValue,
} from "../src/index.js";

const repository = fileURLToPath(new URL("../../../..", import.meta.url));

function context(toolName: string, callId: string): DynToolCallContext {
  return {
    session_id: "parity-session",
    tool_name: toolName,
    call_id: callId,
    state_attributes: {},
    secrets: {},
  };
}

function buildFixture(): void {
  execFileSync(
    "cargo",
    [
      "build",
      "--quiet",
      "--release",
      "--target",
      "wasm32-unknown-unknown",
      "-p",
      "aomi-wasm-parity",
      "--lib",
    ],
    { cwd: repository, stdio: "inherit" },
  );
}

function nativeSnapshot(): JsonValue {
  return JSON.parse(
    execFileSync(
      "cargo",
      ["run", "--quiet", "-p", "aomi-wasm-parity", "--bin", "aomi-wasm-parity-native"],
      { cwd: repository, encoding: "utf8" },
    ),
  ) as JsonValue;
}

function collectAsync(plugin: AomiWasmPlugin, executionId: number): {
  updates: AsyncExecPoll[];
  afterComplete: AsyncExecPoll;
} {
  const updates: AsyncExecPoll[] = [];
  for (let attempts = 0; attempts < 100; attempts += 1) {
    const poll = plugin.poll(executionId);
    if (poll.status === "pending") {
      continue;
    }
    assert.equal(poll.status, "update");
    updates.push(poll);
    if (!poll.has_more) {
      return { updates, afterComplete: plugin.poll(executionId) };
    }
  }
  throw new Error("WebAssembly async parity timed out");
}

test("TypeScript WebAssembly binding matches the native Rust ABI", async () => {
  buildFixture();
  const native = nativeSnapshot();
  const wasmPath = new URL(
    "../../../../target/wasm32-unknown-unknown/release/aomi_wasm_parity.wasm",
    import.meta.url,
  );
  const plugin = await AomiWasmPlugin.instantiate(await readFile(wasmPath));

  try {
    const manifest = plugin.manifest();
    const greet = plugin.startTool("greet", { name: "Ada" }, context("greet", "greet-1"));
    const invalidArgs = plugin.startTool(
      "greet",
      { name: 42 },
      context("greet", "invalid-1"),
    );
    const unknownTool = plugin.startTool(
      "missing",
      {},
      context("missing", "missing-1"),
    );
    const asyncStart = plugin.startTool("count", { upto: 3 }, context("count", "count-1"));
    assert.equal(asyncStart.status, "async_queued");
    const { updates, afterComplete } = collectAsync(plugin, asyncStart.execution_id);
    const cancelStart = plugin.startTool("count", { upto: 3 }, context("count", "cancel-1"));
    assert.equal(cancelStart.status, "async_queued");
    const cancelActive = plugin.cancel(cancelStart.execution_id);
    const pollCanceled = plugin.poll(cancelStart.execution_id);
    const pollAfterCancel = plugin.poll(cancelStart.execution_id);

    const wasm = {
      sdk_version: plugin.sdkVersion,
      manifest,
      greet,
      invalid_args: invalidArgs,
      unknown_tool: unknownTool,
      async_start: asyncStart,
      async_updates: updates,
      poll_after_complete: afterComplete,
      cancel_start: cancelStart,
      cancel_active: cancelActive,
      poll_canceled: pollCanceled,
      poll_after_cancel: pollAfterCancel,
      cancel_unknown: plugin.cancel(999_999),
    };

    assert.deepEqual(wasm, native);
  } finally {
    plugin.dispose();
  }
});
