// Runs the BUILT script wasm (web target, pkg/) under Node and checks
// it against the committed CLI fixtures — end-to-end parity of the
// actual artifact, not just the native-compiled crate that
// tests/cli_parity.rs covers.
//
//   wasm-pack build --target web --release
//   node check_node_parity.mjs
import { readFile } from "node:fs/promises";
import init, * as script from "./pkg/fflame_script.js";

const here = (p) => new URL(p, import.meta.url);
await init({ module_or_path: await readFile(here("pkg/fflame_script_bg.wasm")) });

let failed = false;
const check = (name, ok) => {
  console.log(`${ok ? "ok  " : "FAIL"} ${name}`);
  if (!ok) failed = true;
};

const list = JSON.parse(script.list_scripts());
check(`library lists ${list.length} scripts (>= 10)`, list.length >= 10);
check("turntable is flagged norng", list.find((s) => s.id === "turntable")?.flags.norng === true);

const gen = script.script_source("basic_random");
const env = JSON.parse(script.run(gen, 7n, "{}"));
const fx1 = await readFile(here("tests/fixtures/basic_random_seed7.fflame"), "utf8");
check("generator byte-identical to CLI fixture", env.config_json === fx1);

const jitter = script.script_source("jitter");
const env2 = JSON.parse(script.run_on(jitter, 7n, "{}", env.config_json));
const fx2 = await readFile(here("tests/fixtures/basic_random_seed7_jitter.fflame"), "utf8");
check("modifier-on-base byte-identical to CLI fixture", env2.config_json === fx2);

const turntable = script.script_source("turntable");
const env3 = JSON.parse(script.run(turntable, 1n, "{}"));
check("script-defined animation rides in the envelope", typeof env3.animation_json === "string");

process.exit(failed ? 1 : 0);
