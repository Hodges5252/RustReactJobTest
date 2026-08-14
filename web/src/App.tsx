import { useEffect, useState } from "react";
import init, { Sim } from "./wasm/sim_core";
import CityCanvas, { extractGeometry, type CityGeometry } from "./components/CityCanvas";
import SeedBar from "./components/SeedBar";
import { getOrCreateSeed, randomSeed, setSeedInUrl } from "./seed";

interface CityState {
  seed: bigint;
  geometry: CityGeometry;
}

export default function App() {
  const [wasmReady, setWasmReady] = useState(false);
  const [city, setCity] = useState<CityState | null>(null);

  useEffect(() => {
    let cancelled = false;
    init().then(() => {
      if (!cancelled) setWasmReady(true);
    });
    return () => {
      cancelled = true;
    };
  }, []);

  useEffect(() => {
    if (!wasmReady) return;
    buildCity(getOrCreateSeed());
  }, [wasmReady]);

  const buildCity = (seed: bigint) => {
    const sim = new Sim(seed);
    const geometry = extractGeometry(sim);
    sim.free();
    setCity({ seed, geometry });
  };

  const regenerate = () => {
    const seed = randomSeed();
    setSeedInUrl(seed);
    buildCity(seed);
  };

  return (
    <div className="app">
      <header className="top-bar">
        <span className="app-title">City Traffic Simulator</span>
        {city && <SeedBar seed={city.seed} onRegenerate={regenerate} />}
      </header>
      {city ? (
        <CityCanvas geometry={city.geometry} />
      ) : (
        <div className="loading">generating city…</div>
      )}
    </div>
  );
}
