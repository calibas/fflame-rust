// Infinite Gallery — proof of concept.
//
// The two wasm modules do all the work: fflame-script turns
// (generator, rooms, seed) into a FractalConfig, fflame-render turns
// the config into pixels. This file is scroll + URL glue, deliberately
// minimal — the real gallery lives in its own repo.
//
// Position model: a hallway is a generator script; image n of the
// hallway is the generator run with seed n, then each applied room
// (modifier script) folded on top with the SAME seed n. The URL hash
// carries (hallway, rooms+params, first visible seed), so a link
// reproduces the exact images.

import initScript, * as script from "../../wasm/script/pkg/fflame_script.js";
import initRender, * as render from "../../wasm/render/pkg/fflame_render.js";

const TILE = 512;
const TILE_ITERS = 30_000_000;   // user-editable in the full version
const BIG = 1024;
const BIG_ITERS = 120_000_000;

const $ = (id) => document.getElementById(id);
const state = {
  scripts: [],          // from list_scripts()
  hall: "basic_random",
  rooms: [],            // [{id, params: {key: value}}]
  seed: 0,              // first tile's seed
  sources: new Map(),   // id -> source
  epoch: 0,             // bumped on any state change; stale renders drop
};

// ---------------------------------------------------------------- URL

function readHash() {
  const h = new URLSearchParams(location.hash.replace(/^#/, ""));
  if (h.get("hall")) state.hall = h.get("hall");
  state.seed = Math.max(0, parseInt(h.get("seed") ?? "0", 10) || 0);
  state.rooms = (h.get("rooms") ?? "").split(",").filter(Boolean).map((r) => {
    const i = r.indexOf(":");
    return i < 0
      ? { id: r, params: {} }
      : { id: r.slice(0, i), params: JSON.parse(decodeURIComponent(r.slice(i + 1))) };
  });
}

function writeHash() {
  const h = new URLSearchParams();
  h.set("hall", state.hall);
  h.set("seed", String(state.seed));
  if (state.rooms.length) {
    h.set("rooms", state.rooms.map((r) =>
      Object.keys(r.params).length
        ? `${r.id}:${encodeURIComponent(JSON.stringify(r.params))}`
        : r.id
    ).join(","));
  }
  history.replaceState(null, "", "#" + h.toString());
}

// ------------------------------------------------- config + rendering

function sourceOf(id) {
  if (!state.sources.has(id)) state.sources.set(id, script.script_source(id));
  return state.sources.get(id);
}

// (generator, rooms, seed n) -> config JSON. Every stage runs with the
// same hallway seed — the deliberate, documented convention.
function configFor(seed) {
  let env = JSON.parse(script.run(sourceOf(state.hall), BigInt(seed), "{}"));
  for (const room of state.rooms) {
    env = JSON.parse(script.run_on(
      sourceOf(room.id), BigInt(seed), JSON.stringify(room.params), env.config_json));
  }
  return env.config_json;
}

// One render at a time: the GPU is a shared resource and WebGPU won't
// meaningfully parallelize compute from one tab anyway.
let queueTail = Promise.resolve();
function enqueue(job) {
  queueTail = queueTail.then(job, job);
  return queueTail;
}

async function renderTile(tileEl, seed) {
  const epoch = state.epoch;
  await enqueue(async () => {
    if (epoch !== state.epoch || !tileEl.isConnected) return; // stale
    try {
      const config = configFor(seed);
      const t = await render.render(config, TILE, TILE, TILE_ITERS);
      if (epoch !== state.epoch) return;
      const canvas = document.createElement("canvas");
      canvas.width = t.width; canvas.height = t.height;
      canvas.getContext("2d").putImageData(
        new ImageData(new Uint8ClampedArray(t.pixels), t.width, t.height), 0, 0);
      canvas.title = `seed ${seed} — ${(t.ms / 1000).toFixed(2)}s`;
      canvas.onclick = () => showBig(seed);
      tileEl.replaceChildren(canvas, seedLabel(seed));
    } catch (e) {
      tileEl.replaceChildren(Object.assign(document.createElement("div"),
        { className: "err", textContent: `seed ${seed}: ${e}` }));
    }
  });
}

function seedLabel(seed) {
  const s = document.createElement("div");
  s.className = "seed"; s.textContent = `#${seed}`;
  return s;
}

async function showBig(seed) {
  const overlay = $("overlay"), canvas = $("big");
  overlay.classList.add("open");
  canvas.width = canvas.height = BIG;
  canvas.getContext("2d").clearRect(0, 0, BIG, BIG);
  const epoch = state.epoch;
  await enqueue(async () => {
    if (epoch !== state.epoch || !overlay.classList.contains("open")) return;
    const t = await render.render(configFor(seed), BIG, BIG, BIG_ITERS);
    canvas.getContext("2d").putImageData(
      new ImageData(new Uint8ClampedArray(t.pixels), t.width, t.height), 0, 0);
  });
}
$("overlay").onclick = () => $("overlay").classList.remove("open");

// --------------------------------------------------- the endless grid

const grid = $("grid");
let nextSeed = 0;

const watcher = new IntersectionObserver((entries) => {
  for (const e of entries) {
    if (!e.isIntersecting) continue;
    watcher.unobserve(e.target);
    const seed = Number(e.target.dataset.seed);
    renderTile(e.target, seed);
    topUp(seed);
  }
}, { rootMargin: "600px" });

function addTile(seed) {
  const el = document.createElement("div");
  el.className = "tile";
  el.dataset.seed = seed;
  el.append(Object.assign(document.createElement("div"),
    { className: "wait", textContent: `seed ${seed}` }));
  grid.append(el);
  watcher.observe(el);
}

function topUp(revealedSeed) {
  // Keep a screenful of tiles ahead of the deepest one revealed.
  while (nextSeed < revealedSeed + 12) addTile(nextSeed++);
}

function rebuild() {
  state.epoch++;
  grid.replaceChildren();
  nextSeed = state.seed;
  for (let i = 0; i < 12; i++) addTile(nextSeed++);
  writeHash();
}

// ------------------------------------------------------------- header

function roomParamInputs(entry) {
  // Param editors for the selected room, from the script's declared
  // params. Values are collected on "Walk through".
  const box = $("room-params");
  box.replaceChildren();
  for (const p of entry?.params ?? []) {
    const label = document.createElement("label");
    label.textContent = p.label;
    let input;
    if (p.type === "choice") {
      input = document.createElement("select");
      p.options.forEach((o, i) => input.add(new Option(o, o, i === p.default, i === p.default)));
    } else {
      input = document.createElement("input");
      if (p.type === "float" || p.type === "int") {
        input.type = "number"; input.value = p.default;
        if (p.type === "float") input.step = "any";
      } else if (p.type === "bool") {
        input.type = "checkbox"; input.checked = p.default;
      } else if (p.type === "color") {
        input.type = "color";
        input.value = "#" + p.default.map((c) =>
          Math.round(c * 255).toString(16).padStart(2, "0")).join("");
      } else {
        input.type = "text"; input.value = p.default;
      }
    }
    input.dataset.key = p.key; input.dataset.ptype = p.type;
    label.append(input);
    box.append(label);
  }
}

function collectRoomParams() {
  const params = {};
  for (const input of $("room-params").querySelectorAll("[data-key]")) {
    const t = input.dataset.ptype;
    params[input.dataset.key] =
      t === "float" ? parseFloat(input.value) :
      t === "int" ? parseInt(input.value, 10) :
      t === "bool" ? input.checked :
      input.value; // choice by name, color as "#rrggbb", text as-is
  }
  return params;
}

function renderAppliedRooms() {
  const box = $("applied");
  box.replaceChildren();
  state.rooms.forEach((room, i) => {
    const chip = document.createElement("span");
    chip.className = "room-chip";
    chip.textContent = room.id;
    const x = document.createElement("button");
    x.textContent = "×"; x.title = "go back through this room";
    x.onclick = () => { state.rooms.splice(i, 1); renderAppliedRooms(); rebuild(); };
    chip.append(x);
    box.append(chip);
  });
}

function wireHeader() {
  const hallSel = $("hall"), roomSel = $("room");
  for (const s of state.scripts) {
    if (s.kind === "generator") hallSel.add(new Option(s.name, s.id));
    else roomSel.add(new Option(s.name, s.id));
  }
  hallSel.value = state.hall;
  if (!hallSel.value) { hallSel.selectedIndex = 0; state.hall = hallSel.value; }
  const roomEntry = () => state.scripts.find((s) => s.id === roomSel.value);
  roomParamInputs(roomEntry());

  hallSel.onchange = () => { state.hall = hallSel.value; rebuild(); };
  roomSel.onchange = () => roomParamInputs(roomEntry());
  $("enter").onclick = () => {
    state.rooms.push({ id: roomSel.value, params: collectRoomParams() });
    renderAppliedRooms();
    rebuild();
  };
  $("seed").value = state.seed;
  $("seed").onchange = () => {
    state.seed = Math.max(0, parseInt($("seed").value, 10) || 0);
    rebuild();
  };
}

// -------------------------------------------------------------- start

(async () => {
  try {
    await Promise.all([initScript(), initRender()]);
    await render.probe();
  } catch (e) {
    $("fatal").hidden = false;
    $("fatal").textContent =
      `Could not start: ${e}. This page needs WebGPU (Chrome/Edge 113+, Firefox 121+) ` +
      `and must be served over http from the repo root after building both wasm modules.`;
    $("status").textContent = "failed";
    return;
  }
  state.scripts = JSON.parse(script.list_scripts());
  readHash();
  wireHeader();
  renderAppliedRooms();
  $("status").textContent = `${TILE}×${TILE} @ ${(TILE_ITERS / 1e6)}M iters`;
  rebuild();
})();
