import { COLORS, CONGESTION_STOPS } from "./CityCanvas";

const ZONE_LABELS = ["Residential", "Commercial", "Industrial"];

const congestionGradient = `linear-gradient(90deg, ${CONGESTION_STOPS.map(
  ([r, g, b]) => `rgb(${r},${g},${b})`,
).join(", ")})`;

/** Small key for zone colors and the road congestion ramp (spec UPDATE 2.6). */
export default function Legend() {
  return (
    <div className="legend">
      {ZONE_LABELS.map((label, i) => (
        <span className="legend-item" key={label}>
          <span className="legend-swatch" style={{ background: COLORS.zones[i] }} />
          {label}
        </span>
      ))}
      <span className="legend-item">
        <span
          className="legend-swatch legend-gradient"
          style={{ background: congestionGradient }}
        />
        Road congestion (free&nbsp;→&nbsp;heavy)
      </span>
    </div>
  );
}
