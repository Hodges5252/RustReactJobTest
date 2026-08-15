import { useEffect, useMemo, useRef } from "react";
import type { Sim } from "../wasm/sim_core";
import type { FrameData } from "../hooks/useSimulation";

const CANVAS_SIZE = 900;

export const COLORS = {
  background: "#f7f4ee",
  roadLocal: "#d6d0c6",
  roadArterial: "#cdc6ba",
  zones: ["#a5c6e5", "#f2b25c", "#b3aac2"], // residential, commercial, industrial
  // Vehicle dots, slightly deeper than zone fills so they read on roads.
  agents: ["#5d8fc4", "#dd8f2e", "#83779f"],
};

// Neutral -> amber -> soft red congestion ramp (kept calm per spec 2.6).
export const CONGESTION_STOPS: [number, number, number][] = [
  [214, 208, 198], // free flow (neutral road)
  [232, 176, 78], // amber
  [217, 110, 85], // soft warm red
];

/** Map a congestion amount in [0, 1] to a road color. */
export function congestionColor(c: number): string {
  const t = Math.min(Math.max(c, 0), 1) * (CONGESTION_STOPS.length - 1);
  const i = Math.min(Math.floor(t), CONGESTION_STOPS.length - 2);
  const f = t - i;
  const [r1, g1, b1] = CONGESTION_STOPS[i];
  const [r2, g2, b2] = CONGESTION_STOPS[i + 1];
  const r = Math.round(r1 + (r2 - r1) * f);
  const g = Math.round(g1 + (g2 - g1) * f);
  const b = Math.round(b1 + (b2 - b1) * f);
  return `rgb(${r},${g},${b})`;
}

/** Static city geometry pulled once from WASM (it never changes for a seed). */
export interface CityGeometry {
  nodes: Float32Array;
  edges: Uint32Array;
  arterial: Uint8Array;
  /** Zone type per block group (merged blocks are one group). */
  groupZones: Uint8Array;
  /** Offsets into groupOutlineNodes; group g spans [offsets[g], offsets[g+1]). */
  groupOffsets: Uint32Array;
  /** Concatenated outer-boundary node indices per group, clockwise. */
  groupOutlineNodes: Uint32Array;
  worldSize: number;
}

export function extractGeometry(sim: Sim): CityGeometry {
  return {
    nodes: sim.node_positions(),
    edges: sim.edge_endpoints(),
    arterial: sim.edge_arterial(),
    groupZones: sim.block_group_zones(),
    groupOffsets: sim.block_group_outline_offsets(),
    groupOutlineNodes: sim.block_group_outline_nodes(),
    worldSize: sim.world_size(),
  };
}

interface Props {
  geometry: CityGeometry;
  subscribe: (fn: (frame: FrameData) => void) => () => void;
}

export default function CityCanvas({ geometry, subscribe }: Props) {
  const canvasRef = useRef<HTMLCanvasElement>(null);

  const transform = useMemo(() => {
    const pad = geometry.worldSize * 0.07;
    const scale = CANVAS_SIZE / (geometry.worldSize + 2 * pad);
    return { pad, scale };
  }, [geometry]);

  useEffect(() => {
    const canvas = canvasRef.current;
    if (!canvas) return;
    const dpr = window.devicePixelRatio || 1;
    canvas.width = CANVAS_SIZE * dpr;
    canvas.height = CANVAS_SIZE * dpr;
    const ctx = canvas.getContext("2d")!;
    ctx.setTransform(dpr, 0, 0, dpr, 0, 0);

    drawFrame(ctx, geometry, transform, null);
    return subscribe((frame) => drawFrame(ctx, geometry, transform, frame));
  }, [geometry, transform, subscribe]);

  return (
    <canvas
      ref={canvasRef}
      className="city-canvas"
      style={{ width: CANVAS_SIZE, height: CANVAS_SIZE }}
    />
  );
}

interface Transform {
  pad: number;
  scale: number;
}

/** Gutter between a block's fill and the surrounding roads, world units. */
const BLOCK_INSET = 13;

/** Road stroke widths (canvas px). Wide enough that a lane-offset vehicle
 * dot in each direction sits fully inside the road. */
const ROAD_WIDTH_LOCAL = 12;
const ROAD_WIDTH_ARTERIAL = 15;
/** Vehicle dot radius (canvas px); small enough for two opposing lanes. */
const VEHICLE_RADIUS = 2.8;

function drawFrame(
  ctx: CanvasRenderingContext2D,
  geo: CityGeometry,
  t: Transform,
  frame: FrameData | null,
): void {
  const wx = (v: number) => (v + t.pad) * t.scale;
  const { nodes, edges, arterial, groupZones, groupOffsets, groupOutlineNodes } = geo;

  ctx.fillStyle = COLORS.background;
  ctx.fillRect(0, 0, CANVAS_SIZE, CANVAS_SIZE);

  // Roads: clean lines with rounded joins/caps (spec 2.6).
  ctx.lineCap = "round";
  ctx.lineJoin = "round";
  for (let i = 0; i < edges.length / 2; i++) {
    const a = edges[i * 2];
    const b = edges[i * 2 + 1];
    const isArterial = arterial[i] === 1;
    // speed_factor spans 1.0 (free) down to 0.2 (jammed); normalize to [0,1].
    const factor = frame ? frame.edgeFactors[i] : 1;
    const congestion = (1 - factor) / 0.8;
    ctx.strokeStyle =
      congestion > 0.02
        ? congestionColor(congestion)
        : isArterial
          ? COLORS.roadArterial
          : COLORS.roadLocal;
    ctx.lineWidth = isArterial ? ROAD_WIDTH_ARTERIAL : ROAD_WIDTH_LOCAL;
    ctx.beginPath();
    ctx.moveTo(wx(nodes[a * 2]), wx(nodes[a * 2 + 1]));
    ctx.lineTo(wx(nodes[b * 2]), wx(nodes[b * 2 + 1]));
    ctx.stroke();
  }

  // Zone block groups: each group (single cell or 2-3 merged cells) is one
  // combined shape with no interior road line, inset by a fixed distance so
  // roads stay visible in the gutters regardless of block size.
  for (let g = 0; g < groupZones.length; g++) {
    const start = groupOffsets[g];
    const end = groupOffsets[g + 1];
    const outline: [number, number][] = [];
    for (let k = start; k < end; k++) {
      const n = groupOutlineNodes[k];
      outline.push([wx(nodes[n * 2]), wx(nodes[n * 2 + 1])]);
    }

    ctx.fillStyle = COLORS.zones[groupZones[g]] ?? COLORS.zones[0];
    roundedPolygon(ctx, insetPolygon(outline, BLOCK_INSET * t.scale), 10);
    ctx.fill();
  }

  // Vehicles: simple dots colored by destination zone type.
  if (frame) {
    const { agents } = frame;
    for (let i = 0; i < agents.length / 3; i++) {
      const x = wx(agents[i * 3]);
      const y = wx(agents[i * 3 + 1]);
      const zone = agents[i * 3 + 2];
      ctx.fillStyle = COLORS.agents[zone] ?? COLORS.agents[0];
      ctx.beginPath();
      ctx.arc(x, y, VEHICLE_RADIUS, 0, Math.PI * 2);
      ctx.fill();
    }
  }
}

function roundedPolygon(
  ctx: CanvasRenderingContext2D,
  pts: readonly (readonly [number, number])[],
  radius: number,
): void {
  const n = pts.length;
  ctx.beginPath();
  for (let i = 0; i < n; i++) {
    const [px, py] = pts[i];
    const [nx, ny] = pts[(i + 1) % n];
    if (i === 0) {
      ctx.moveTo((px + nx) / 2, (py + ny) / 2);
    }
    ctx.arcTo(nx, ny, pts[(i + 2) % n][0], pts[(i + 2) % n][1], radius);
  }
  ctx.closePath();
}

/**
 * Shrink a clockwise polygon inward by a fixed distance: every edge is
 * offset along its inward normal and consecutive offset edges re-intersected.
 * Works for the (possibly non-convex) merged-block outlines.
 */
function insetPolygon(
  pts: readonly (readonly [number, number])[],
  d: number,
): [number, number][] {
  const n = pts.length;
  const out: [number, number][] = [];
  for (let i = 0; i < n; i++) {
    const p0 = pts[(i + n - 1) % n];
    const p1 = pts[i];
    const p2 = pts[(i + 1) % n];

    // Edge directions and inward normals (clockwise polygon, y-down coords).
    const d1x = p1[0] - p0[0];
    const d1y = p1[1] - p0[1];
    const l1 = Math.hypot(d1x, d1y) || 1;
    const d2x = p2[0] - p1[0];
    const d2y = p2[1] - p1[1];
    const l2 = Math.hypot(d2x, d2y) || 1;
    const n1x = (-d1y / l1) * d;
    const n1y = (d1x / l1) * d;
    const n2x = (-d2y / l2) * d;
    const n2y = (d2x / l2) * d;

    // Intersect the two offset edge lines.
    const a1x = p0[0] + n1x;
    const a1y = p0[1] + n1y;
    const a2x = p1[0] + n2x;
    const a2y = p1[1] + n2y;
    const cross = d1x * d2y - d1y * d2x;
    if (Math.abs(cross) < 1e-6) {
      // Nearly collinear edges: fall back to a simple normal offset.
      out.push([p1[0] + n1x, p1[1] + n1y]);
    } else {
      const s = ((a2x - a1x) * d2y - (a2y - a1y) * d2x) / cross;
      out.push([a1x + d1x * s, a1y + d1y * s]);
    }
  }
  return out;
}
