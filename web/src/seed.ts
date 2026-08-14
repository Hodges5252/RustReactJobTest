const U64_MAX = 2n ** 64n;

export function randomSeed(): bigint {
  const words = new Uint32Array(2);
  crypto.getRandomValues(words);
  return (BigInt(words[0]) << 32n) | BigInt(words[1]);
}

export function setSeedInUrl(seed: bigint): void {
  const url = new URL(window.location.href);
  url.searchParams.set("seed", seed.toString());
  window.history.replaceState(null, "", url);
}

/** Read `?seed=` from the URL; if absent/invalid, generate one and reflect it
 *  into the URL so a refresh reproduces the same city (spec 2.7). */
export function getOrCreateSeed(): bigint {
  const raw = new URLSearchParams(window.location.search).get("seed");
  if (raw && /^\d+$/.test(raw)) {
    const value = BigInt(raw);
    if (value < U64_MAX) {
      return value;
    }
  }
  const seed = randomSeed();
  setSeedInUrl(seed);
  return seed;
}
