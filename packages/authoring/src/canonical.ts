export function canonicalJson(value: unknown): string {
  return JSON.stringify(canonicalValue(value));
}

export function stableFingerprint(value: unknown): string {
  const bytes = new TextEncoder().encode(canonicalJson(value));
  let hash = 0xcbf29ce484222325n;
  for (const byte of bytes) {
    hash ^= BigInt(byte);
    hash = BigInt.asUintN(64, hash * 0x100000001b3n);
  }
  return `fnv1a64:${hash.toString(16).padStart(16, '0')}`;
}

export function immutable<Value>(value: Value): Value {
  if (value === null || typeof value !== 'object') return value;
  const record = value as Record<string, unknown>;
  for (const child of Object.values(record)) immutable(child);
  return Object.freeze(value);
}

function canonicalValue(value: unknown): unknown {
  if (Array.isArray(value)) return value.map(canonicalValue);
  if (value !== null && typeof value === 'object') {
    return Object.fromEntries(
      Object.entries(value as Readonly<Record<string, unknown>>)
        .sort(([left], [right]) => left.localeCompare(right))
        .map(([key, child]) => [key, canonicalValue(child)]),
    );
  }
  return value;
}
