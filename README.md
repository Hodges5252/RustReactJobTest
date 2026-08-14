# City Traffic Simulator

A procedurally-generated city traffic simulator that runs entirely in the browser.
The simulation core (city generation, road network, Dijkstra pathfinding, agent
scheduling, and congestion modeling) is written in **Rust** and compiled to
**WebAssembly**; the presentation layer is a **React + TypeScript + Vite** app that
renders the live simulation to an HTML canvas. The whole thing is a static site —
no backend, no database, no runtime API calls — deployed to **GitHub Pages** via
GitHub Actions.

- A seeded 9x9 city is generated on load: commercial core, residential ring,
  industrial edge clusters, connected road grid with jittered intersections.
- 700 residents each get a home and a workplace, depart in a randomized
  morning-peak wave (~7:30 AM) and return in an evening wave (~5:30 PM),
  producing 100+ concurrent trips and emergent rush-hour jams.
- Congested roads slow traffic (`speed_factor = clamp(1 - load/capacity * 0.8, 0.2, 1.0)`)
  and shift color from neutral gray through amber to warm red.
- The day runs 6:00 AM to 11:00 PM (about 7 real minutes at 1x), then loops with
  freshly-sampled departure times.
- Share a city via the `?seed=` URL parameter — the same seed always produces
  the identical city.

## Prerequisites

| Tool | Version | Install |
| --- | --- | --- |
| Rust toolchain | stable (1.97+) | [rustup.rs](https://rustup.rs) |
| `wasm32-unknown-unknown` target | — | `rustup target add wasm32-unknown-unknown` |
| wasm-pack | 0.13+ | [Releases](https://github.com/wasm-bindgen/wasm-pack/releases) or `cargo install wasm-pack` |
| Node.js | 20+ | [nodejs.org](https://nodejs.org) |

## Building the Rust/WASM core

From the repository root:

```bash
wasm-pack build sim-core --target web --release --out-dir ../web/src/wasm
```

This compiles the `sim-core` crate to WebAssembly and writes the JS/TS bindings
into `web/src/wasm/`, where the React app imports them. Re-run it whenever you
change Rust code. (From inside `web/` you can also run `npm run build:wasm`.)

## Running the React app locally

```bash
cd web
npm install
npm run build:wasm   # only needed after Rust changes / first run
npm run dev
```

Open the printed URL (default `http://localhost:5173`). A random seed is
generated and reflected into the URL; use **Copy link** to share the exact city
or **New city** to regenerate.

To produce a production build locally: `npm run build` (output in `web/dist/`,
preview with `npm run preview`).

## Running the tests

Automated unit tests cover generation determinism, pathfinding validity, full
road-network connectivity, the congestion formula, and peak scheduling:

```bash
cd sim-core
cargo test
```

The manual QA checklist for visual/interactive behavior is in
[`QA_CHECKLIST.md`](QA_CHECKLIST.md).

## How the deployment pipeline works

[`.github/workflows/deploy.yml`](.github/workflows/deploy.yml) runs on every
push to `main` (and via manual dispatch):

1. **Build job**
   - Installs stable Rust with the `wasm32-unknown-unknown` target and a pinned
     wasm-pack binary (cargo registry and `sim-core/target` are cached).
   - Runs `wasm-pack build sim-core --target web --release` into `web/src/wasm/`.
   - Runs `npm ci && npm run build` in `web/`, with `BASE_PATH=/<repo-name>/` so
     Vite's `base` matches the GitHub Pages project path (see `web/vite.config.ts`).
   - Uploads `web/dist/` with `actions/upload-pages-artifact`.
2. **Deploy job** publishes the artifact with `actions/deploy-pages` to the
   repository's GitHub Pages environment.

## Enabling & verifying GitHub Pages

One-time repository setup:

1. Push this repository to GitHub with `main` as the default branch.
2. Go to **Settings → Pages** and set **Source** to **GitHub Actions**
   (not "Deploy from a branch").
3. Push any commit to `main` (or run the workflow from the **Actions** tab via
   "Run workflow").

To verify a deployment:

1. Open the **Actions** tab and confirm the latest "Deploy to GitHub Pages" run
   is green; the deploy job shows the published URL.
2. The site is served at `https://<your-username>.github.io/<repo-name>/`.
3. Load it, then run through [`QA_CHECKLIST.md`](QA_CHECKLIST.md) — in
   particular confirm the URL gains a `?seed=` parameter and that reloading it
   reproduces the same city.

## Project layout

```
├── .github/workflows/deploy.yml   # build WASM + Vite app, deploy to GitHub Pages
├── sim-core/                      # Rust crate compiled to WebAssembly
│   ├── src/
│   │   ├── lib.rs                 # wasm-bindgen exports: Sim, tick, typed-array state accessors
│   │   ├── rng.rs                 # hand-rolled SplitMix64 seeded PRNG
│   │   ├── city.rs                # grid generation, jitter, rule-based zoning
│   │   ├── graph.rs               # road network graph, congestion speed factors
│   │   ├── pathfinding.rs         # Dijkstra over current edge weights
│   │   ├── agents.rs              # agent pool, home/work assignment, peak-window sampling
│   │   └── simulation.rs          # tick loop, departures, movement, day cycle
│   ├── examples/smoke.rs          # native full-day sanity run with stats
│   └── tests/core_tests.rs        # determinism, pathfinding, connectivity, congestion tests
└── web/                           # React + TypeScript + Vite app
    └── src/
        ├── hooks/useSimulation.ts # owns the Sim instance + requestAnimationFrame loop
        ├── components/
        │   ├── CityCanvas.tsx     # canvas rendering of zones/roads/vehicles/congestion
        │   ├── TimeControls.tsx   # play/pause, 1x/2x/4x, time-of-day readout
        │   ├── StatsPanel.tsx     # active trips, average travel time
        │   └── SeedBar.tsx        # seed display, copy-link, regenerate
        ├── wasm/                  # wasm-pack output (generated, gitignored)
        └── seed.ts                # ?seed= URL read/write helpers
```

## Performance notes

Per-frame state crosses the WASM/JS boundary as flat `Float32Array`/`Uint32Array`
buffers (positions, destination zones, edge speed factors) — no JSON
serialization. At the target scale (~180 road segments, 100-200 concurrent
vehicles) a full canvas redraw per frame comfortably sustains 60fps on a typical
modern laptop.
