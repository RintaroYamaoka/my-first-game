import { z } from "zod";
import { log } from "./log";

const Id = z.string().regex(/^f-\d{4}$/);

export function parse(url: string | undefined): string | null {
  const m = /^\/fragment\/([^/]+)$/.exec(url ?? "");
  if (!m) return null;
  const id = Id.safeParse(m[1]);
  if (!id.success) log(`bad id: ${m[1]}`);
  return id.success ? id.data : null;
}

export function parsePort(value: unknown): number {
  const port = Number(value);
  if (!Number.isInteger(port)) log(`bad port: ${value}`);
  return port;
}
