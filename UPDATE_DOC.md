# UPDATE_DOC.md — City Traffic Simulator: Realism & Variety Pass

This is an addendum to `BUILD_DOC.md` — the base project is already implemented and working (all 32 original rubric criteria pass); this document specifies incremental improvements on top of it. Read both documents; nothing here removes or contradicts the original spec unless explicitly stated.

---

## Part 1 — High-Level Update Prompt

Improve the existing city traffic simulator's realism and visual variety without changing its core architecture (Rust/WASM simulation core, React/TypeScript canvas rendering, static GitHub Pages deployment, seeded determinism). Five improvements are in scope. First, fix vehicle rendering so cars traveling in opposite directions on the same road no longer render on top of each other (a per-direction lane offset), and so cars traveling the same direction on the same segment maintain a minimum following distance rather than overlapping or passing through one another. Second, make the generated road network feel less uniformly grid-like by adding a handful of genuine dead-end stub roads and pruning a portion of redundant local streets, in both cases without ever breaking full connectivity between the original intersections. Third, allow 2-3 adjacent same-zone grid cells to merge into a single larger rectangular block (oriented horizontally or vertically, with the shared interior road removed), so not every block is a uniform single-cell square; as a stretch goal only, also support merging 3 cells into a non-rectangular ("tromino"/L-shaped) block if it does not meaningfully threaten the other items' completion. Fourth, add life to the middle of the day and late evening by giving a configurable fraction of agents an extra midday errand trip (work to a commercial block and back) and/or evening errand trip (home to a commercial block and back), distinct from their morning/evening commute, so the city doesn't go quiet for hours at a time. Fifth, replace the current instantaneous "vehicle vanishes exactly at the destination intersection corner" behavior with a short, visible "driveway" approach: vehicles should visibly pull out of a block's interior onto the road at departure, and visibly pull from the road into a block's interior at arrival, rather than popping in/out exactly at a corner node. Finally, add a small, unobtrusive UI legend clarifying what the zone colors and the road congestion color ramp mean.

Explicitly out of scope for this update (do not build): full intersection right-of-way/stop-sign/traffic-light simulation, general non-rectangular/arbitrary polygon block shapes beyond the optional tromino stretch goal, any change to the deployment/hosting model, and any regression of the 32 quality-rubric criteria already established in `BUILD_DOC.md`. Prioritize finishing items 1, 2 (dead-ends + pruning), 3 (rectangular merges only), 4, and 5 in full before attempting the tromino stretch goal in item 3 or any further polish.

---

## Part 2 — Detailed Specifications

### 2.1 Lane Offset & Following-Distance Spacing
- Every active agent's rendered position must be offset perpendicular to its current direction of travel (the vector from its current edge's "from" node to its "to" node) by a small fixed amount, consistently to one side (e.g., always the right-hand side relative to travel direction), so that opposite-direction traffic on the same road segment renders as two visually distinct lanes rather than one overlapping line.
- Recommended constant: `LANE_OFFSET = 6.0` world units (tunable; world units follow the existing `CELL_SIZE = 100.0` scale, so this is a subtle, proportionate offset). Bake this offset directly into the position computation in `sim-core/src/simulation.rs` (`trip_position`) so the frontend does not need to duplicate any geometry logic.
- Within a single road segment, agents traveling in the same direction must maintain a minimum following distance and must never visually overlap or pass through one another. Implement this as a per-tick pass that, for each edge, groups currently-traversing agents by direction, orders them by progress along the edge, and clamps each trailing agent's progress so it cannot close to less than a minimum gap behind the agent ahead of it on the same edge and direction.
- Recommended constant: `MIN_FOLLOW_GAP = 10.0` world units (tunable).
- This is a lightweight follow-the-leader clamp, not a full traffic simulation: it must not introduce deadlock (an agent should never become permanently stuck because the leader ahead of it is itself stuck — since leaders are processed/ordered by progress first, followers simply inherit the leader's effective position minus the gap, so this resolves naturally).
- Full intersection stop-sign/traffic-light/right-of-way logic is explicitly out of scope for this update.

### 2.2 Organic Road Network: Dead-Ends & Pruning
- After building the base grid graph (`Graph::build`), run a post-processing pass that:
  1. Adds a small number of genuine dead-end stub roads: pick a handful of random existing intersections (recommended: 3-6) and attach a new leaf node to each via a short new edge (recommended stub length: 40-60% of `CELL_SIZE`), positioned so it doesn't visually overlap the existing grid. Since a leaf addition never disconnects anything, this requires no connectivity check and is guaranteed safe.
  2. Prunes a portion of redundant local (non-arterial) edges for variety: attempt removal of up to ~10% of local edges (arterial edges must never be pruned), each time verifying via a full BFS/connectivity check that the graph remains fully connected after removal; revert (keep the edge) if removal would disconnect any node. Never prune an edge that forms the shared interior boundary of a merged block from 2.3 (those are removed separately and deliberately).
- The road network must remain fully connected among all of the original grid intersections after this pass, per the existing rubric requirement; the new dead-end leaf nodes are expected to only be reachable via their single stub (that is the point of a dead end) and are not required to have redundant connectivity.

### 2.3 Merged Rectangular Blocks (+ Optional Tromino Stretch Goal)
- After zone assignment, run a merge pass that identifies eligible adjacent same-zone cell groups and merges some of them into a single larger block:
  - Eligible shapes: 2 cells in a straight line (1x2 or 2x1) or 3 cells in a straight line (1x3 or 3x1) — horizontal or vertical only. Blocks remain rectangular.
  - Recommended merge rate: roughly 20% of eligible cells end up part of a merge (tunable); prefer variety (some 2-cell and some 3-cell merges) over uniformity.
  - When cells are merged, remove the shared interior road edge(s) between them (subject to the same connectivity-preservation check as 2.2's pruning — skip the merge if removing that edge would disconnect the graph, though this should essentially never trigger since the perimeter remains intact).
  - Render merged blocks as a single combined rectangular shape (the bounding rectangle of the merged cells' outer corners) with no interior road line drawn through it, instead of drawing each cell as a separate quad.
  - Expose whatever new data the frontend needs (e.g., a block-group id per cell, or the merged group's outer corner list) via new `sim-core/src/lib.rs` WASM getters, following the existing flat-array convention (no JSON).
- Stretch goal only, attempt after everything else in this document is complete and working: support one non-rectangular 3-cell ("tromino"/L-shaped) merge configuration, rendered by computing the merged group's outer boundary (the set of unit cell edges that belong to exactly one cell in the group) and drawing that boundary as a canvas path instead of a simple rectangle. If this proves time-consuming, skip it — it does not block considering this update complete.

### 2.4 Midday & Evening Errand Trips
- Extend the agent state machine (currently `AtHome → ToWork → AtWork → ToHome → DoneForDay` in `sim-core/src/agents.rs`) to support errand trips:
  - Midday errand: a configurable fraction of agents (recommended ~15%) depart from work to a randomly chosen commercial block and back, departing in a midday window (recommended ~11:30 AM - 1:30 PM), returning to work the same day within roughly 30-60 simulated minutes.
  - Evening errand: a configurable fraction of agents (recommended ~15%, may overlap with or differ from the midday group) depart from home to a randomly chosen commercial block and back, in a window after their commute home (recommended ~7:00 PM - 9:30 PM), returning home before day end (11:00 PM).
  - Reuse the existing pathfinding/trip/movement machinery for errand legs — an errand trip is just another origin-destination pair routed the same way a commute is.
- These additions must not cause the concurrently-active agent count to regularly exceed the original 100-200 target during the existing morning/evening commute peaks (the goal is filling in the quiet hours, not inflating peak load).

### 2.5 Departure/Arrival "Driveway" Approach
- Replace the instantaneous appear/disappear-at-the-corner-node behavior with a short visible approach at both ends of every trip (commute or errand):
  - On departure, the agent should visibly move from the origin block's interior (its centroid) out to the corner intersection node before starting normal road travel.
  - On arrival, the agent should visibly move from the destination corner intersection node into the destination block's interior (its centroid) before disappearing.
  - Recommended implementation: model each as a short synthetic "driveway leg" prepended/appended to the trip's node/edge sequence, distinct from real graph edges (it has no congestion/load and is not part of `Graph`), traveled at a fixed slow speed over a fixed short distance (recommended: 20% of `CELL_SIZE`, i.e. ~20 world units, at a fixed speed distinctly slower than `LOCAL_BASE_SPEED`).
  - This should be a self-contained addition to `sim-core/src/agents.rs` (the `Trip`/`Phase` model) and `sim-core/src/simulation.rs` (movement/arrival logic) that does not change how real road edges/congestion work.

### 2.6 Color Legend
- Add a small, unobtrusive legend to the UI (a new component, e.g. `web/src/components/Legend.tsx`) that displays:
  - A swatch and label for each zone type (residential, commercial, industrial) matching the colors used in `CityCanvas.tsx`.
  - A small gradient swatch (or a couple of discrete swatches) illustrating the road congestion color ramp from free-flow to congested, with a brief label (e.g., "road congestion").
- Keep it visually consistent with the existing minimal, flat, calm aesthetic (see `BUILD_DOC.md` 2.6) — it should read as part of the same clean UI, not a bolted-on overlay.

### 2.7 Non-Goals for This Update
Do not build any of the following as part of this update:
- Full intersection right-of-way, stop-sign, or traffic-light simulation.
- General arbitrary/non-rectangular block shapes beyond the single optional tromino stretch goal in 2.3.
- Any change to the deployment model, hosting, or the client-side-only/no-backend architecture.
- Any regression of the 32 quality-rubric criteria already defined in `BUILD_DOC.md`.

---

## Part 3 — Recommended Framework & File Structure

### Files expected to change
- `sim-core/src/city.rs` — zone assignment gains a block-merge grouping pass (2.3); dead-end/pruning candidates may also be identified here or in `graph.rs`.
- `sim-core/src/graph.rs` — `Graph::build` becomes aware of merged-block interior edges to skip; add a post-build pass for dead-end stub addition and local-edge pruning (2.2), each connectivity-checked.
- `sim-core/src/agents.rs` — extend `Phase`/`Trip` to support errand phases (2.4) and driveway legs (2.5).
- `sim-core/src/simulation.rs` — lane offset baked into `trip_position` (2.1); following-distance clamping added to `move_agents` (2.1); errand scheduling added to `process_departures` (2.4); driveway leg handling in movement/arrival logic (2.5).
- `sim-core/src/lib.rs` — new WASM getters as needed for merged-block outlines/groups (2.3); existing getters (`node_positions`, `edge_endpoints`, `agent_states`, etc.) should continue to work unchanged for dead-end nodes/edges since they're just additional entries in the same flat arrays.
- `sim-core/tests/core_tests.rs` — new tests: connectivity still holds after dead-end/pruning/merge generation; following-distance is respected; errand trips occur within their configured windows for a sample seed.
- `web/src/components/CityCanvas.tsx` — render merged-block outlines instead of per-cell quads where a block group applies; no change needed for lane offset since it's baked into the position data already received.
- `web/src/components/Legend.tsx` (new) — the color/congestion legend (2.6).
- `web/src/App.tsx` — mount the new `Legend` component.
- `QA_CHECKLIST.md` — append new manual checks (see below).
- `README.md` — brief mention of the realism/variety pass (low priority, not required for this update to be considered complete).

### Recommended Build Order
1. **Traffic realism (2.1)**: lane offset first (pure rendering-data change, low risk, immediately visible improvement), then following-distance spacing.
2. **Road network variety (2.2)**: dead-end stubs (simplest, zero connectivity risk) then local-edge pruning (connectivity-checked).
3. **Merged rectangular blocks (2.3)**: cell-merge grouping, interior edge removal, merged-outline rendering. Stop here unless time remains, then optionally attempt the tromino stretch goal.
4. **Errand trips (2.4)**: extend the state machine, verify midday/evening activity via the existing native `examples/smoke.rs`-style sanity check.
5. **Driveway approach (2.5)**: synthetic departure/arrival legs.
6. **Color legend (2.6)**: UI-only, can be done independently/in parallel with any of the above.
7. **Testing & QA**: extend automated tests per file list above; update `QA_CHECKLIST.md`; re-run the full original manual QA checklist from `BUILD_DOC.md` to confirm no regressions.

### Manual QA Checklist additions (append to `QA_CHECKLIST.md`)
- [ ] Opposite-direction traffic on the same road renders as two visually separated lanes, not one overlapping line.
- [ ] Same-direction vehicles on a congested road visibly queue/space out rather than overlapping or passing through each other.
- [ ] At least a few genuine dead-end stub roads are visible and do not connect through to another intersection.
- [ ] The city layout includes at least one visibly larger merged block (spanning 2-3 grid cells) rather than every block being a uniform single square.
- [ ] Some vehicles are visibly active during the midday quiet period and during the late-evening period, not only during the two commute rush hours.
- [ ] Vehicles visibly pull out of a block onto the road when departing, and pull from the road into a block when arriving, rather than popping in/out exactly at an intersection corner.
- [ ] The legend correctly identifies each zone color and the congestion color ramp.
- [ ] All original `BUILD_DOC.md` manual QA checklist items still pass (no regressions).

---

## Part 4 — Quality Rubric

Each item is a single, atomic pass/fail criterion, additive to (not a replacement for) the 32 criteria in `BUILD_DOC.md`.

1. Vehicles render with a perpendicular lane offset based on direction of travel, so opposite-direction traffic on the same road segment is visually separated.
2. Vehicles traveling the same direction on the same road segment maintain a minimum following distance and do not visually overlap or pass through one another.
3. The generated road network includes at least 3 genuine dead-end stub roads that do not connect through to another intersection.
4. Adding dead-end stubs does not reduce connectivity among the original grid intersections.
5. A subset of local (non-arterial) edges is removed from the road network for variety, verified to preserve full connectivity among the original grid intersections.
6. Arterial edges are never removed by the pruning pass.
7. City generation supports merging 2 or 3 adjacent same-zone grid cells into a single larger rectangular block, oriented horizontally or vertically.
8. Merged blocks render as a single combined shape with no interior road line drawn through them.
9. A typical default-seed generated city includes at least one merged block of 2 or more cells.
10. (Optional/stretch) If implemented, non-rectangular 3-cell merged blocks render with a correct outline matching the merged cells' boundary.
11. A configurable fraction of agents take a midday errand trip to a commercial block and back, distinct from their commute.
12. A configurable fraction of agents take an evening errand trip to a commercial block and back after arriving home, distinct from their commute.
13. For a typical seed, the count of active trips is greater than zero at multiple sampled points during the midday window (approximately 11 AM-2 PM).
14. For a typical seed, the count of active trips is greater than zero at multiple sampled points during the late-evening window (approximately 7-10 PM).
15. Adding errand trips does not cause the concurrently active agent count to regularly exceed the original 100-200 target during the morning or evening commute peaks.
16. When a vehicle departs a block, its rendered position visibly moves from the block's interior to the road rather than appearing instantaneously at the intersection corner.
17. When a vehicle arrives at its destination, its rendered position visibly moves from the intersection corner into the block's interior before disappearing, rather than vanishing exactly at the corner.
18. The UI includes a legend identifying what each zone color represents.
19. The UI includes a legend or key explaining the road congestion color ramp.
20. All automated tests present before this update continue to pass unmodified in their assertions (no weakening of existing test expectations).
21. At least one new automated test verifies road network connectivity holds after dead-end, pruning, and merge generation.
22. The manual QA checklist is updated to include the new checks listed in Part 3, and all of them pass.
23. None of the 32 quality-rubric criteria defined in `BUILD_DOC.md` regress as a result of this update.
