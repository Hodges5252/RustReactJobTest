# BUILD_DOC.md — City Traffic Simulator (Rust + React, static/WASM)

This document is a complete build specification for a procedurally-generated city traffic simulator. It is written to be handed directly to a coding model/agent with no additional context needed.

---

## Part 1 — High-Level Project Prompt

Build a procedurally-generated city traffic simulator that runs entirely in the browser with no backend server. The simulation core (city generation, road network, pathfinding, agent movement, scheduling, and congestion modeling) is written in Rust and compiled to WebAssembly; the presentation layer is a React + TypeScript application that loads the WASM module, drives a `requestAnimationFrame` loop, and renders the live simulation to an HTML canvas. The entire application is a static site (HTML/JS/WASM only, no server-side code, no database, no external API calls at runtime) and is deployed automatically to GitHub Pages via a GitHub Actions workflow triggered on push to `main`.

On load, the app deterministically generates a small city (an 8x8 to 10x10 grid of blocks) from a seed value: a connected road network and at least three zone types (residential, commercial, industrial) placed using simple rule-based logic (not pure randomness — e.g. a commercial core, residential ring, industrial edge cluster, with light randomized variation). The seed is read from a `?seed=` URL query parameter; if absent, a random seed is generated and reflected into the URL so the exact same city can be reproduced or shared via link. A population of agents (target 100-200 concurrently active) is generated, each assigned a home zone and a work zone. Agents travel along the road network using pathfinding (Dijkstra or A*), departing for work in a randomized morning-peak window and departing for home in a randomized evening-peak window, simulating a full day from 6:00 AM to 11:00 PM. As roads accumulate traffic load, their effective travel speed drops and this is visually reflected (road color shifts to indicate congestion), producing organic, emergent rush-hour traffic jams rather than scripted ones. The UI provides play/pause, at least two speed multipliers, a display of the current simulated time of day, and a small live stats readout (e.g. active trips, average travel time).

Visually, the project should take inspiration from the clean, minimalist, flat-color aesthetic of the game *Mini Motorways* (without copying its specific art assets or trademarks): solid flat colors, simple geometric shapes for zones/buildings, clean rounded-line roads, simple dot/shape vehicles, generous whitespace, and minimal UI chrome. It should feel calm, legible, and a little charming rather than technical or cluttered.

The repository must include a comprehensive `README.md` covering: prerequisites (Rust toolchain, `wasm-pack`, Node.js), how to build the Rust/WASM core, how to run the React app locally in dev mode, how the GitHub Actions deployment pipeline works, and how to enable/verify GitHub Pages for the repo (including any required repo settings). The project must be buildable and demoable within a single day, so scope is intentionally minimal — see Non-Goals below — and should prioritize reaching a working, deployed, end-to-end demo early, then layering features incrementally (see the Recommended Build Order in Part 3).

---

## Part 2 — Detailed Specifications

### 2.1 City Generation & Zoning
- Generate a grid of blocks (default: 9x9, configurable constant, must stay within 8x8–10x10).
- Apply light randomized jitter to intersection positions (small, bounded) so the layout doesn't look perfectly robotic, while keeping the underlying graph a simple planar grid.
- Assign zone types per block using a rule-based radial pattern: commercial core near the grid center, residential forming a ring around it, industrial clustered at one or two edges/corners. Mix in bounded randomness so it's not perfectly symmetric.
- At least 3 distinct zone types must exist: residential, commercial, industrial.
- All generation must be driven by a single seeded PRNG (see 2.7) — no use of non-deterministic randomness anywhere in generation.

### 2.2 Road Network & Pathfinding
- Represent the road network as a graph: intersections are nodes, road segments are weighted edges (weight = current travel time, derived from length and current congestion, see 2.4).
- The graph must be fully connected — every block must be reachable from every other block.
- Optionally vary road class (arterial vs. local street) with different base capacity/width for visual and gameplay variety — this is a nice-to-have, not required for v1 correctness.
- Implement pathfinding via Dijkstra or A* over this graph. Agents compute a route from their current location to their destination using current edge weights.

### 2.3 Traffic Agents & Scheduling
- Each agent has: a home zone (block), a work zone (block), and a daily schedule.
- Morning departure time: sampled from a randomized distribution centered around a morning peak (e.g. ~7:30 AM) with reasonable spread.
- Evening departure time: sampled similarly around an evening peak (e.g. ~5:30 PM).
- Optionally include light background/errand trips during the day for realism (nice-to-have, not required for v1).
- Agents not currently traveling are considered "at home" or "at work" and are not rendered/counted toward the active concurrent agent budget.
- Total resident pool may exceed the concurrently-active target; only agents currently mid-trip count toward the 100-200 concurrent target (see 2.9).

### 2.4 Congestion Model
- Each road segment tracks current load (number of agents currently on it) versus a capacity value.
- Use a simple, explicit linear slowdown formula (do not over-engineer):
  `speed_factor = clamp(1 - (current_load / capacity) * 0.8, 0.2, 1.0)`
  `effective_speed = base_speed * speed_factor`
- Recompute edge weights (travel time = segment_length / effective_speed) periodically (e.g. every simulation tick or every few ticks) so pathfinding reacts to congestion.
- Speed factor must never reach zero (floor at 0.2) to avoid simulation deadlock.

### 2.5 Time-of-Day & Simulation Controls
- Simulate a full day from 6:00 AM to 11:00 PM (17 in-simulation hours).
- Default real-time duration for one full simulated day at 1x speed: roughly 5-10 minutes. Provide at least a 2x and 4x speed multiplier.
- Provide play/pause control.
- Display current simulated time of day in the UI at all times.
- When the simulated day ends (11:00 PM), loop back to 6:00 AM with a freshly-scheduled day (agents keep the same home/work assignment; only departure-time sampling re-rolls), unless time runs out to implement — looping is a nice-to-have; stopping/resetting at day end is an acceptable v1 fallback.

### 2.6 Visual Style & Rendering
- Rendering target: HTML5 Canvas 2D (WebGL/3D libraries are explicitly out of scope — unnecessary complexity for this scale).
- Flat, solid colors; avoid gradients, shadows, and textures beyond very subtle accents.
- Zones rendered as simple rounded-rectangle blocks, color-coded by type (e.g., soft blue = residential, warm orange = commercial, muted gray/purple = industrial), with visible spacing between blocks.
- Roads rendered as clean lines with rounded joins/caps; neutral color when uncongested.
- Congestion indicated by shifting road color along a neutral-to-warm gradient (e.g., gray to amber to red) as load increases — avoid harsh/alarming colors, keep it feeling calm.
- Vehicles rendered as small simple shapes (circle or rounded rect), color matched to the zone type of their current destination.
- Background: soft off-white/neutral tone.
- UI chrome: a single slim bar (top or bottom) containing time-of-day readout, play/pause, speed toggle, seed display + "copy link"/regenerate button, and a compact live stats readout (active trips, average travel time). Minimal padding-heavy, uncluttered layout; simple sans-serif type.
- Agent movement and any UI transitions should use smooth interpolation/easing rather than discrete jumps.

### 2.7 Seed System & Shareability
- All procedural generation and randomized scheduling must derive from a single seeded PRNG. Recommend implementing a small deterministic PRNG by hand (e.g. splitmix64/xorshift) rather than relying on the `rand` crate's OS-randomness features, to avoid WebAssembly `getrandom` configuration friction under time pressure.
- Seed is a plain integer (e.g. u64), passed via `?seed=<number>` in the URL.
- On load with no seed param, generate a random seed and update the URL (e.g. via `history.replaceState`) so refreshing reproduces the same city.
- Provide a UI affordance to copy the current shareable URL and to regenerate a new random city.

### 2.8 Deployment & Hosting
- No backend server, database, or runtime API calls of any kind.
- Build pipeline: `wasm-pack build --target web` (or equivalent) compiles the Rust crate to a WASM package consumed by the Vite/React app; Vite bundles the final static site.
- GitHub Actions workflow (triggered on push to `main`) builds the WASM package, builds the Vite app, and deploys the output via the official GitHub Pages Actions (`actions/upload-pages-artifact` + `actions/deploy-pages`).
- Vite `base` config must be set correctly for the GitHub Pages project path.
- README must document how to enable GitHub Pages (Settings → Pages → Source: GitHub Actions) and how to verify the deployed URL.

### 2.9 Scale & Performance Targets
- City grid: 8x8 to 10x10 blocks (default 9x9).
- Concurrently active/rendered agents: target 100-200 during peak periods.
- Target frame rate: sustain ~60fps, acceptable floor of 30fps, on a typical modern laptop in a current Chrome/Edge/Firefox browser.
- Avoid full JSON (de)serialization of simulation state every frame; prefer flat typed-array (e.g. `Float32Array`) hand-offs between WASM and JS for per-frame agent/road state to keep overhead low at this scale.

### 2.10 Non-Goals / Explicit Exclusions
Do NOT build any of the following — they are explicitly out of scope for this project:
- No backend server, database, or persistent storage of any kind.
- No user accounts, authentication, or per-user saved state.
- No multiplayer or real-time networking between multiple users/clients.
- No mobile/touch-specific optimization (desktop browser only).
- No sound or audio.
- No photorealistic, textured, or 3D graphics — 2D flat-color only.
- No terrain, elevation, water, or other geographic features.
- No pedestrians, public transit, or non-vehicle agents.
- No traffic light/signal logic beyond whatever is minimally implied by the congestion model (a full signal-phase simulation is out of scope).
- No saving/loading of manually edited cities — only regeneration via seed.
- No automated end-to-end/UI test suite (browser automation, visual regression, etc.).

### 2.11 Testing Strategy
- Automated: include Rust unit tests (`#[test]`) for the core deterministic logic — at minimum: (a) city generation produces an identical result for the same seed across two runs, (b) pathfinding returns a valid, connected route between two arbitrary blocks, (c) the congestion speed_factor formula produces expected outputs at known load/capacity inputs.
- Manual QA: maintain a short manual QA checklist (see Part 3) covering visual/interactive behaviors not worth automating given the timeline (rendering correctness, control responsiveness, visual congestion feedback, URL seed round-trip).

---

## Part 3 — Recommended Framework & File Structure

### Repository layout
```
/
├── README.md
├── .gitignore
├── .github/
│   └── workflows/
│       └── deploy.yml          # build wasm + vite app, deploy to GitHub Pages
├── sim-core/                   # Rust crate compiled to WebAssembly
│   ├── Cargo.toml
│   ├── src/
│   │   ├── lib.rs              # wasm-bindgen exports: init(seed), tick(dt), state accessors
│   │   ├── rng.rs              # hand-rolled seeded PRNG (splitmix64/xorshift)
│   │   ├── city.rs             # grid generation, zoning rules
│   │   ├── graph.rs            # road network graph (nodes/edges, weights)
│   │   ├── pathfinding.rs      # Dijkstra/A* implementation
│   │   ├── agents.rs           # agent struct, home/work assignment, schedule sampling
│   │   └── simulation.rs       # main tick loop, congestion recalculation, state snapshot
│   └── tests/
│       └── core_tests.rs       # determinism, pathfinding validity, congestion formula
└── web/                        # React + TypeScript + Vite app
    ├── package.json
    ├── vite.config.ts
    ├── tsconfig.json
    ├── index.html
    ├── public/
    └── src/
        ├── main.tsx
        ├── App.tsx
        ├── wasm/                # wasm-pack output consumed here (or npm-linked package)
        ├── hooks/
        │   └── useSimulation.ts # requestAnimationFrame loop, calls into wasm, reads state
        ├── components/
        │   ├── CityCanvas.tsx   # canvas rendering of roads/zones/agents
        │   ├── TimeControls.tsx # play/pause/speed
        │   ├── StatsPanel.tsx   # active trips, avg travel time
        │   └── SeedBar.tsx      # seed display, copy-link, regenerate
        └── styles/
            └── global.css
```

### Toolchain notes
- Rust → WASM via `wasm-pack build --target web`.
- Frontend via Vite + React + TypeScript.
- Deployment via GitHub Actions using the official GitHub Pages actions (Pages source set to "GitHub Actions" in repo settings).

### Recommended Build Order (phased, each phase ends in a demoable state)
1. **Scaffolding & pipeline first**: minimal Rust crate that compiles to WASM and returns a dummy value, minimal React app that loads it and renders a blank canvas, GitHub Actions workflow deploying this "hello world" to a live GitHub Pages URL. Validate the full pipeline works before building any simulation features.
2. **Static city generation & rendering**: seeded grid/road/zone generation in Rust, rendered in the Mini-Motorways-style flat visual design in React. No agents yet. Seed-in-URL working.
3. **Pathfinding & basic movement**: Dijkstra/A* implemented, a handful of agents animate smoothly from home to work along computed routes.
4. **Full scheduling**: generate the full agent population, morning/evening peak departure sampling, run a full simulated day 6 AM–11 PM with play/pause/speed controls.
5. **Congestion model**: road load tracking, speed_factor slowdown, visual congestion coloring on roads.
6. **Polish**: stats panel, time-of-day display, spacing/typography pass to match the visual spec.
7. **Testing & QA**: write the Rust unit tests, run through the manual QA checklist, verify performance at the target agent count.
8. **Final deployment check**: confirm the live GitHub Pages URL reflects the finished build; finalize README.

### Manual QA Checklist (to include in the repo, e.g. as `QA_CHECKLIST.md` or a README section)
- [ ] Loading the app with no seed param generates a city and updates the URL with a seed.
- [ ] Reloading the same URL reproduces an identical city layout.
- [ ] Every zone block is reachable by road from every other zone block (visually spot-check a few routes).
- [ ] Agents visibly depart in a morning wave and an evening wave, not uniformly throughout the day.
- [ ] Roads visibly change color under heavy load and return to normal once load subsides.
- [ ] Play, pause, and both speed multipliers behave correctly.
- [ ] The time-of-day display accurately reflects simulation progress from 6 AM to 11 PM.
- [ ] The app sustains acceptable frame rate (no visible stutter) at the target concurrent agent count.
- [ ] The deployed GitHub Pages URL loads and functions identically to local dev.

---

## Part 4 — Quality Rubric

Each item is a single, atomic pass/fail criterion.

1. The project includes a Rust crate that implements the simulation logic and compiles to WebAssembly.
2. The project includes a React + TypeScript frontend that renders the simulation.
3. The application runs entirely client-side with no backend server or runtime API calls.
4. The application is deployable as a static site via GitHub Pages.
5. The repository includes a GitHub Actions workflow that automatically builds and deploys to GitHub Pages on push to `main`.
6. The README includes complete step-by-step instructions for running the project locally.
7. The README includes complete step-by-step instructions for deploying to GitHub Pages.
8. The application procedurally generates a city consisting of a road network and zoned blocks.
9. The generated city includes at least three distinct zone types (residential, commercial, industrial).
10. Zone placement follows a discernible rule-based pattern rather than uniform randomness.
11. City generation is fully deterministic given a seed value.
12. The seed is readable from and writable to a `?seed=` URL query parameter.
13. Loading the same seeded URL twice produces an identical city.
14. The road network graph is fully connected (every block reachable from every other).
15. Traffic agents use a pathfinding algorithm (Dijkstra or A*) to compute routes.
16. Each agent is assigned a distinct home zone and work zone.
17. Agents depart for work in a randomized morning-peak window.
18. Agents depart for home in a randomized evening-peak window.
19. The simulation models a full day cycle from 6:00 AM to 11:00 PM.
20. Increased traffic load on a road segment measurably reduces agent travel speed on that segment.
21. Road segments visually indicate their current congestion level.
22. The UI provides play and pause controls.
23. The UI provides at least two distinct simulation speed multipliers.
24. The UI displays the current simulated time of day at all times.
25. The default city grid is between 8x8 and 10x10 blocks.
26. The simulation supports at least 100 concurrently active agents while sustaining at least 30fps on a typical modern laptop browser.
27. The visual style uses flat colors and simple geometric shapes, with no photorealistic or 3D-textured graphics.
28. The project contains no user authentication, accounts, or persistent per-user data storage.
29. The project contains no multiplayer or real-time networking between clients.
30. The Rust codebase includes automated unit tests covering city generation determinism and pathfinding validity.
31. The repository includes a documented manual QA checklist distinct from the automated tests.
32. The repository's code is organized into the top-level `sim-core/` and `web/` directories as specified in Part 3.
