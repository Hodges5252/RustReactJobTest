import { useEffect, useMemo, useRef } from "react";
import type { Sim } from "../wasm/sim_core";

const CANVAS_SIZE = 900;

const COLORS = {
  background: "#f7f4ee",
  roadLocal: "#d6d0c6",
  roadArterial: "#cdc6ba",
  zones: ["#a5c6e5", "#f2b25c", "#b3aac2"], // residential, commercial, industrial
};

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
}

export default function CityCanvas({ geometry }: Props) {
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
    ctx.scale(dpr, dpr);
    drawCity(ctx, geometry, transform);
  }, [geometry, transform]);

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

function drawCity(
  ctx: CanvasRenderingContext2D,
  geo: CityGeometry,
  t: Transform,
): void {
  const wx = (x: number) => (x + t.pad) * t.scale;
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
    ctx.strokeStyle = isArterial ? COLORS.roadArterial : COLORS.roadLocal;
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
