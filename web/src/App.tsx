import { useEffect, useRef, useState } from "react";
import init, { pipeline_check } from "./wasm/sim_core";

export default function App() {
  const canvasRef = useRef<HTMLCanvasElement>(null);
  const [wasmStatus, setWasmStatus] = useState("loading WASM…");

  useEffect(() => {
    let cancelled = false;
    init().then(() => {
      if (!cancelled) {
        setWasmStatus(`WASM loaded, pipeline_check() = ${pipeline_check()}`);
      }
    });
    return () => {
      cancelled = true;
    };
  }, []);

  return (
    <div className="app">
      <div className="status-bar">{wasmStatus}</div>
      <canvas ref={canvasRef} width={900} height={900} className="city-canvas" />
    </div>
  );
}
