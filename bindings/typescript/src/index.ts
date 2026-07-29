export type JsonPrimitive = boolean | number | string | null;
export type JsonValue = JsonPrimitive | JsonValue[] | { [key: string]: JsonValue };

export interface DynToolCallContext {
  session_id: string;
  tool_name: string;
  call_id: string;
  state_attributes?: Record<string, JsonValue>;
  secrets?: Record<string, string>;
}

export interface DynToolMetadata {
  name: string;
  app: string;
  description: string;
  parameters_schema: JsonValue;
  supports_async: boolean;
  namespace?: string;
}

export interface SecretSlot {
  name: string;
  description: string;
  required: boolean;
}

export interface BroadcastConfig {
  default: string;
  allowed: string[];
}

export interface DynManifest {
  sdk_version: string;
  name: string;
  version: string;
  preamble: string;
  tools: DynToolMetadata[];
  namespaces?: string[];
  secrets?: SecretSlot[];
  broadcast?: BroadcastConfig;
}

export type DynToolResult = { Ok: JsonValue } | { Err: string };

export type DynToolStart =
  | { status: "ready"; result: DynToolResult }
  | { status: "async_queued"; execution_id: number };

export type AsyncExecPoll =
  | { status: "pending" }
  | { status: "update"; value: JsonValue; has_more: boolean }
  | { status: "error"; message: string }
  | { status: "canceled" }
  | { status: "not_found" };

export interface DynExecCancel {
  canceled: boolean;
}

type AomiWasmExports = WebAssembly.Exports & {
  memory: WebAssembly.Memory;
  aomi_sdk_version(): number;
  aomi_create(): number;
  aomi_manifest(instance: number): number;
  aomi_async_tool_start(
    instance: number,
    name: number,
    argsJson: number,
    contextJson: number,
  ): number;
  aomi_dyn_exec_poll(instance: number, executionId: bigint): number;
  aomi_dyn_exec_cancel(instance: number, executionId: bigint): number;
  aomi_destroy(instance: number): void;
  aomi_free_string(pointer: number): void;
  aomi_alloc(length: number): number;
  aomi_dealloc(pointer: number, length: number): void;
};

interface Allocation {
  pointer: number;
  length: number;
}

const requiredExports = [
  "memory",
  "aomi_sdk_version",
  "aomi_create",
  "aomi_manifest",
  "aomi_async_tool_start",
  "aomi_dyn_exec_poll",
  "aomi_dyn_exec_cancel",
  "aomi_destroy",
  "aomi_free_string",
  "aomi_alloc",
  "aomi_dealloc",
] as const;

/**
 * TypeScript owner for one macro-generated Aomi plugin compiled to
 * `wasm32-unknown-unknown`.
 *
 * The class speaks the same JSON ABI as the native `DynFnHandle`; it only
 * replaces C-string transport with reads and writes against exported WASM
 * memory.
 */
export class AomiWasmPlugin {
  readonly sdkVersion: string;

  private readonly exports: AomiWasmExports;
  private instance: number;

  private constructor(exports: AomiWasmExports) {
    this.exports = exports;
    this.instance = exports.aomi_create();
    if (this.instance === 0) {
      throw new Error("aomi_create returned a null instance");
    }
    this.sdkVersion = this.readCString(exports.aomi_sdk_version());
  }

  static async instantiate(
    source: BufferSource | WebAssembly.Module,
    imports: WebAssembly.Imports = {},
  ): Promise<AomiWasmPlugin> {
    const instance =
      source instanceof WebAssembly.Module
        ? await WebAssembly.instantiate(source, imports)
        : (await WebAssembly.instantiate(source, imports)).instance;
    const exports = instance.exports as AomiWasmExports;

    for (const name of requiredExports) {
      if (!(name in exports)) {
        throw new Error(`Aomi WebAssembly module is missing export '${name}'`);
      }
    }
    if (!(exports.memory instanceof WebAssembly.Memory)) {
      throw new Error("Aomi WebAssembly export 'memory' is not linear memory");
    }

    return new AomiWasmPlugin(exports);
  }

  manifest(): DynManifest {
    this.ensureAlive();
    return this.readOwnedJson<DynManifest>(this.exports.aomi_manifest(this.instance));
  }

  startTool(name: string, args: JsonValue, context: DynToolCallContext): DynToolStart {
    return this.startToolJson(name, this.stringify(args), this.stringify(context));
  }

  startToolJson(name: string, argsJson: string, contextJson: string): DynToolStart {
    this.ensureAlive();
    const values = [name, argsJson, contextJson];
    if (values.some((value) => value.includes("\0"))) {
      throw new TypeError("Aomi ABI strings cannot contain NUL bytes");
    }
    const inputs = values.map((value) => this.allocateCString(value));
    try {
      return this.readOwnedJson<DynToolStart>(
        this.exports.aomi_async_tool_start(
          this.instance,
          inputs[0]!.pointer,
          inputs[1]!.pointer,
          inputs[2]!.pointer,
        ),
      );
    } finally {
      for (const input of inputs) {
        this.exports.aomi_dealloc(input.pointer, input.length);
      }
    }
  }

  poll(executionId: number): AsyncExecPoll {
    this.ensureAlive();
    return this.readOwnedJson<AsyncExecPoll>(
      this.exports.aomi_dyn_exec_poll(this.instance, this.executionId(executionId)),
    );
  }

  cancel(executionId: number): DynExecCancel {
    this.ensureAlive();
    return this.readOwnedJson<DynExecCancel>(
      this.exports.aomi_dyn_exec_cancel(this.instance, this.executionId(executionId)),
    );
  }

  dispose(): void {
    if (this.instance !== 0) {
      this.exports.aomi_destroy(this.instance);
      this.instance = 0;
    }
  }

  private stringify(value: JsonValue | DynToolCallContext): string {
    const json = JSON.stringify(value);
    if (json === undefined) {
      throw new TypeError("Aomi tool input must be JSON-serializable");
    }
    return json;
  }

  private allocateCString(value: string): Allocation {
    const encoded = new TextEncoder().encode(value);
    const length = encoded.byteLength + 1;
    const pointer = this.exports.aomi_alloc(length);
    if (pointer === 0) {
      throw new Error(`aomi_alloc failed for ${length} bytes`);
    }
    const memory = new Uint8Array(this.exports.memory.buffer, pointer, length);
    memory.set(encoded);
    memory[encoded.byteLength] = 0;
    return { pointer, length };
  }

  private readOwnedJson<T>(pointer: number): T {
    if (pointer === 0) {
      throw new Error("Aomi WebAssembly plugin returned a null JSON pointer");
    }
    try {
      return JSON.parse(this.readCString(pointer)) as T;
    } finally {
      this.exports.aomi_free_string(pointer);
    }
  }

  private readCString(pointer: number): string {
    if (pointer === 0) {
      throw new Error("Aomi WebAssembly plugin returned a null string pointer");
    }
    const memory = new Uint8Array(this.exports.memory.buffer);
    if (pointer >= memory.byteLength) {
      throw new Error("Aomi WebAssembly string pointer is outside linear memory");
    }
    let end = pointer;
    while (end < memory.byteLength && memory[end] !== 0) {
      end += 1;
    }
    if (end === memory.byteLength) {
      throw new Error("Aomi WebAssembly string is not NUL-terminated");
    }
    return new TextDecoder("utf-8", { fatal: true }).decode(memory.subarray(pointer, end));
  }

  private ensureAlive(): void {
    if (this.instance === 0) {
      throw new Error("Aomi WebAssembly plugin has been disposed");
    }
  }

  private executionId(value: number): bigint {
    if (!Number.isSafeInteger(value) || value < 0) {
      throw new TypeError("Aomi execution id must be a non-negative safe integer");
    }
    return BigInt(value);
  }
}
