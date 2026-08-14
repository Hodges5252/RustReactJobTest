const SPEEDS = [1, 2, 4];

export function formatClock(seconds: number): string {
  const h24 = Math.floor(seconds / 3600) % 24;
  const m = Math.floor((seconds % 3600) / 60);
  const ampm = h24 < 12 ? "AM" : "PM";
  const h12 = h24 % 12 === 0 ? 12 : h24 % 12;
  return `${h12}:${m.toString().padStart(2, "0")} ${ampm}`;
}

interface Props {
  clockSec: number;
  playing: boolean;
  onTogglePlay: () => void;
  speed: number;
  onSetSpeed: (speed: number) => void;
}

export default function TimeControls({
  clockSec,
  playing,
  onTogglePlay,
  speed,
  onSetSpeed,
}: Props) {
  return (
    <div className="time-controls">
      <span className="clock">{formatClock(clockSec)}</span>
      <button className="play-pause" onClick={onTogglePlay}>
        {playing ? "Pause" : "Play"}
      </button>
      <div className="speed-group">
        {SPEEDS.map((s) => (
          <button
            key={s}
            className={s === speed ? "speed active" : "speed"}
            onClick={() => onSetSpeed(s)}
          >
            {s}x
          </button>
        ))}
      </div>
    </div>
  );
}
