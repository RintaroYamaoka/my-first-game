// どこからも使われていない
export function legacyId(n: number): string {
  return `f-${String(n).padStart(4, "0")}`;
}
