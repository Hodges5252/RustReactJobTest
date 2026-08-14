import { useState } from "react";

interface Props {
  seed: bigint;
  onRegenerate: () => void;
}

export default function SeedBar({ seed, onRegenerate }: Props) {
  const [copied, setCopied] = useState(false);

  const copyLink = async () => {
    await navigator.clipboard.writeText(window.location.href);
    setCopied(true);
    setTimeout(() => setCopied(false), 1500);
  };

  return (
    <div className="seed-bar">
      <span className="seed-label" title={`Seed: ${seed}`}>
        seed {seed.toString()}
      </span>
      <button onClick={copyLink}>{copied ? "Copied!" : "Copy link"}</button>
      <button onClick={onRegenerate}>New city</button>
    </div>
  );
}
