import { useEffect, useMemo, useRef } from "react";
import type { Sim } from "../wasm/sim_core";
import type { FrameData } from "../hooks/useSimulation";

const CANVAS_SIZE = 900;

const COLORS = {
  background: "#f7f4ee",
  roadLocal: "#d6d0c6",
  roadArterial: "#cdc6ba",
  zones: ["#a5c6e5", "#f2b25c", "#b3aac2"], // residential, commercial, industrial
  // Vehicle dots, slightly deeper than zone fills so they read on roads.
  agents: ["#5d8fc4", "#dd8f2e", "#83779f"],
};

// Neutral -> amber -> soft red congestion ramp (kept calm per spec 2.6).
const CONGESTION_STOPS: [number, number, number][] = [
  [214, 208, 198], // free flow (neutral road)
  [232, 176, 78], // amber
  [217, 110, 85], // soft warm red
];

/** Map a congestion amount in [0, 1] to a road color. */
function congestionColor(c: number): string {
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
  blockZones: Uint8Array;
  blockCorners: Uint32Array;
  worldSize: number;
}

export function extractGeometry(sim: Sim): CityGeometry {
  return {
    nodes: sim.node_positions(),
    edges: sim.edge_endpoints(),
    arterial: sim.edge_arterial(),
    blockZones: sim.block_zones(),
    blockCorners: sim.block_corner_nodes(),
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

function drawFrame(
  ctx: CanvasRenderingContext2D,
  geo: CityGeometry,
  t: Transform,
  frame: FrameData | null,
): void {
  const wx = (v: number) => (v + t.pad) * t.scale;
  const { nodes, edges, arterial, blockZones, blockCorners } = geo;

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
    ctx.lineWidth = isArterial ? 11 : 7;
    ctx.beginPath();
    ctx.moveTo(wx(nodes[a * 2]), wx(nodes[a * 2 + 1]));
    ctx.lineTo(wx(nodes[b * 2]), wx(nodes[b * 2 + 1]));
    ctx.stroke();
  }

  // Zone blocks: inset rounded quads, flat color per zone type.
  for (let b = 0; b < blockZones.length; b++) {
    const corners: [number, number][] = [];
    let cx = 0;
    let cy = 0;
    for (let k = 0; k < 4; k++) {
      const n = blockCorners[b * 4 + k];
      const x = wx(nodes[n * 2]);
      const y = wx(nodes[n * 2 + 1]);
      corners.push([x, y]);
      cx += x / 4;
      cy += y / 4;
    }
    // Inset each corner toward the centroid so roads stay visible between blocks.
    const inset = 0.78;
    const pts = corners.map(
      ([x, y]) => [cx + (x - cx) * inset, cy + (y - cy) * inset] as const,
    );

    ctx.fillStyle = COLORS.zones[blockZones[b]] ?? COLORS.zones[0];
    roundedQuad(ctx, pts, 10);
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
      ctx.arc(x, y, 5, 0, Math.PI * 2);
      ctx.fill();
    }
  }
}

function roundedQuad(
  ctx: CanvasRenderingContext2D,
  pts: readonly (readonly [number, number])[],
  radius: number,
): void {
  ctx.beginPath();
  for (let i = 0; i < 4; i++) {
    const [px, py] = pts[i];
    const [nx, ny] = pts[(i + 1) % 4];
    if (i === 0) {
      ctx.moveTo((px + nx) / 2, (py + ny) / 2);
    }
    ctx.arcTo(nx, ny, pts[(i + 2) % 4][0], pts[(i + 2) % 4][1], radius);
  }
  ctx.closePath();
}
