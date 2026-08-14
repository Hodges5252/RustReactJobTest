import { useCallback, useEffect, useRef, useState } from "react";
import { Sim } from "../wasm/sim_core";
import { extractGeometry, type CityGeometry } from "../components/CityCanvas";

export const DAY_START_SECONDS = 6 * 3600;

/** Per-frame dynamic state handed from WASM as flat typed arrays. */
export interface FrameData {
  /** [x, y, destZone] per active agent. */
  agents: Float32Array;
  /** Congestion speed factor per road segment (1 = free flow). */
  edgeFactors: Float32Array;
}

export interface SimStats {
  activeTrips: number;
  avgTravelTime: number;
}

type FrameListener = (frame: FrameData) => void;

/**
 * Owns the Sim instance and the requestAnimationFrame loop. The sim is
 * created and freed inside one effect so a pending frame can never touch a
 * freed WASM object (also StrictMode-safe).
 */
export function useSimulation(seed: bigint | null, wasmReady: boolean) {
  const [geometry, setGeometry] = useState<CityGeometry | null>(null);
  const [playing, setPlaying] = useState(true);
  const [speed, setSpeed] = useState(1);
  const [clockSec, setClockSec] = useState(DAY_START_SECONDS);
  const [stats, setStats] = useState<SimStats>({ activeTrips: 0, avgTravelTime: 0 });

  const listenerRef = useRef<FrameListener | null>(null);
  const playingRef = useRef(playing);
  playingRef.current = playing;
  const speedRef = useRef(speed);
  speedRef.current = speed;

  useEffect(() => {
    if (!wasmReady || seed === null) return;

    const sim = new Sim(seed);
    setGeometry(extractGeometry(sim));
    setClockSec(sim.clock_seconds());

    let raf = 0;
    let last = performance.now();
    let uiAccum = 0;

    const frame = (now: number) => {
      // Clamp dt so returning from a background tab doesn't jump the sim.
      const dt = Math.min((now - last) / 1000, 0.1);
      last = now;

      if (playingRef.current) {
        sim.tick(dt * speedRef.current);
      }
      listenerRef.current?.({
        agents: sim.agent_states(),
        edgeFactors: sim.edge_speed_factors(),
      });

      // Throttle React state updates; canvas drawing bypasses React entirely.
      uiAccum += dt;
      if (uiAccum >= 0.2) {
        uiAccum = 0;
        setClockSec(sim.clock_seconds());
        setStats({
          activeTrips: sim.active_trip_count(),
          avgTravelTime: sim.avg_travel_time(),
        });
      }

      raf = requestAnimationFrame(frame);
    };
    raf = requestAnimationFrame(frame);

    return () => {
      cancelAnimationFrame(raf);
      sim.free();
    };
  }, [wasmReady, seed]);

  const subscribe = useCallback((fn: FrameListener) => {
    listenerRef.current = fn;
    return () => {
      if (listenerRef.current === fn) listenerRef.current = null;
    };
  }, []);

  return {
    geometry,
    playing,
    setPlaying,
    speed,
    setSpeed,
    clockSec,
    stats,
    subscribe,
  };
}
