/* tslint:disable */
/* eslint-disable */
export function wasm_main(): Promise<void>;
/**
 * Global WASM API instance
 *
 * This is accessed via `window.fractalFlameApi` in JavaScript
 */
export class WasmApi {
  free(): void;
  [Symbol.dispose](): void;
  /**
   * Export PNG from current config
   *
   * Returns PNG data as Uint8Array that can be downloaded or displayed
   *
   * JavaScript usage:
   * ```js
   * const api = new WasmApi();
   * api.load_preset("Bubble");
   * const pngBytes = await api.export_png(800, 600, 256);
   *
   * // Download
   * const blob = new Blob([pngBytes], { type: 'image/png' });
   * const url = URL.createObjectURL(blob);
   * const a = document.createElement('a');
   * a.href = url;
   * a.download = 'fractal.png';
   * a.click();
   * ```
   */
  export_png(width: number, height: number, iterations_per_thread: number): Promise<Uint8Array>;
  /**
   * Check if config is loaded
   */
  has_config(): boolean;
  /**
   * Load built-in preset by name
   *
   * JavaScript usage:
   * ```js
   * api.load_preset("Bubble");
   * ```
   */
  load_preset(name: string): void;
  /**
   * Get render progress (0.0 to 1.0)
   */
  get_progress(): number;
  /**
   * Get current loaded config as JSON
   *
   * JavaScript usage:
   * ```js
   * const configJson = api.get_config_json();
   * console.log(JSON.parse(configJson));
   * ```
   */
  get_config_json(): string;
  /**
   * Get list of available preset names
   *
   * Returns JSON array of preset names
   */
  get_preset_names(): string;
  /**
   * Load config from JSON string
   *
   * JavaScript usage:
   * ```js
   * const api = new WasmApi();
   * api.load_config_json('{"flame": {...}, "max_iterations": 1000000}');
   * ```
   */
  load_config_json(json: string): void;
  /**
   * Load config from URL parameters
   *
   * Supports multiple formats:
   * - `?config=<base64_json>` - Base64-encoded FractalConfig JSON
   * - `?preset=<name>` - Load built-in preset
   *
   * JavaScript usage:
   * ```js
   * const api = new WasmApi();
   * api.load_config_from_url(window.location.href);
   * ```
   */
  load_config_from_url(url_str: string): void;
  /**
   * Get target iteration count
   */
  get_target_iterations(): number;
  /**
   * Set target iterations (useful for testing specific iteration counts)
   */
  set_target_iterations(iterations: number): void;
  /**
   * Get current iteration count
   */
  get_current_iterations(): number;
  /**
   * Create new WASM API instance
   */
  constructor();
}

export type InitInput = RequestInfo | URL | Response | BufferSource | WebAssembly.Module;

export interface InitOutput {
  readonly memory: WebAssembly.Memory;
  readonly wasm_main: () => void;
  readonly __wbg_wasmapi_free: (a: number, b: number) => void;
  readonly wasmapi_export_png: (a: number, b: number, c: number, d: number) => any;
  readonly wasmapi_get_config_json: (a: number) => [number, number, number, number];
  readonly wasmapi_get_current_iterations: (a: number) => number;
  readonly wasmapi_get_preset_names: (a: number) => [number, number];
  readonly wasmapi_get_progress: (a: number) => number;
  readonly wasmapi_get_target_iterations: (a: number) => number;
  readonly wasmapi_has_config: (a: number) => number;
  readonly wasmapi_load_config_from_url: (a: number, b: number, c: number) => [number, number];
  readonly wasmapi_load_config_json: (a: number, b: number, c: number) => [number, number];
  readonly wasmapi_load_preset: (a: number, b: number, c: number) => [number, number];
  readonly wasmapi_new: () => number;
  readonly wasmapi_set_target_iterations: (a: number, b: number) => void;
  readonly __externref_table_alloc: () => number;
  readonly __wbindgen_export_1: WebAssembly.Table;
  readonly __wbindgen_exn_store: (a: number) => void;
  readonly __wbindgen_malloc: (a: number, b: number) => number;
  readonly __wbindgen_realloc: (a: number, b: number, c: number, d: number) => number;
  readonly __wbindgen_free: (a: number, b: number, c: number) => void;
  readonly __wbindgen_export_6: WebAssembly.Table;
  readonly __externref_table_dealloc: (a: number) => void;
  readonly closure1799_externref_shim: (a: number, b: number, c: any) => void;
  readonly closure1886_externref_shim: (a: number, b: number, c: any) => void;
  readonly wasm_bindgen__convert__closures_____invoke__h896ba810511a8433: (a: number, b: number) => void;
  readonly wasm_bindgen__convert__closures_____invoke__h4583a70ca8243b02: (a: number, b: number) => void;
  readonly closure1806_externref_shim: (a: number, b: number, c: any, d: any) => void;
  readonly closure1899_externref_shim: (a: number, b: number, c: any, d: any) => void;
  readonly __wbindgen_start: () => void;
}

export type SyncInitInput = BufferSource | WebAssembly.Module;
/**
* Instantiates the given `module`, which can either be bytes or
* a precompiled `WebAssembly.Module`.
*
* @param {{ module: SyncInitInput }} module - Passing `SyncInitInput` directly is deprecated.
*
* @returns {InitOutput}
*/
export function initSync(module: { module: SyncInitInput } | SyncInitInput): InitOutput;

/**
* If `module_or_path` is {RequestInfo} or {URL}, makes a request and
* for everything else, calls `WebAssembly.instantiate` directly.
*
* @param {{ module_or_path: InitInput | Promise<InitInput> }} module_or_path - Passing `InitInput` directly is deprecated.
*
* @returns {Promise<InitOutput>}
*/
export default function __wbg_init (module_or_path?: { module_or_path: InitInput | Promise<InitInput> } | InitInput | Promise<InitInput>): Promise<InitOutput>;
