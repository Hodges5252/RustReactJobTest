import type { SimStats } from "../hooks/useSimulation";

interface Props {
  stats: SimStats;
}

export default function StatsPanel({ stats }: Props) {
  const avgMinutes = stats.avgTravelTime / 60;
  return (
    <div className="stats-panel">
      <span className="stat">
        <span className="stat-value">{stats.activeTrips}</span> active trips
      </span>
      <span className="stat">
        <span className="stat-value">
          {avgMinutes > 0 ? avgMinutes.toFixed(1) : "–"}
        </span>{" "}
        min avg trip
      </span>
    </div>
  );
}
