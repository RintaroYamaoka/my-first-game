import { existsSync, readFileSync, writeFileSync } from "node:fs";
import type { Fragment } from "./fragment";
import { log } from "./log";

const FILE = "fragments.json";

export const store: Fragment[] = read();

function read(): Fragment[] {
  if (!existsSync(FILE)) return [];
  return JSON.parse(readFileSync(FILE, "utf8"));
}

export function find(id: string): Fragment | null {
  for (const f of store) {
    if (f.id === id) return f;
  }
  return null;
}

export function put(f: Fragment): void {
  store.push(f);
  writeFileSync(FILE, JSON.stringify(store));
  log(`put ${f.id}`);
}
