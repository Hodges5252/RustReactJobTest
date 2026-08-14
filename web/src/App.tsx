import { useEffect, useState } from "react";
import init from "./wasm/sim_core";
import CityCanvas from "./components/CityCanvas";
import SeedBar from "./components/SeedBar";
import { useSimulation } from "./hooks/useSimulation";
import { getOrCreateSeed, randomSeed, setSeedInUrl } from "./seed";

export default function App() {
  const [wasmReady, setWasmReady] = useState(false);
  const [seed, setSeed] = useState<bigint | null>(null);

  useEffect(() => {
    let cancelled = false;
    init().then(() => {
      if (!cancelled) {
        setWasmReady(true);
        setSeed(getOrCreateSeed());
      }
    });
    return () => {
      cancelled = true;
    };
  }, []);

  const sim = useSimulation(seed, wasmReady);

  const regenerate = () => {
    const next = randomSeed();
    setSeedInUrl(next);
    setSeed(next);
  };

  return (
    <div className="app">
      <header className="top-bar">
        <span className="app-title">City Traffic Simulator</span>
        {seed !== null && <SeedBar seed={seed} onRegenerate={regenerate} />}
      </header>
      {sim.geometry ? (
        <CityCanvas geometry={sim.geometry} subscribe={sim.subscribe} />
      ) : (
        <div className="loading">generating city…</div>
      )}
    </div>
  );
}
