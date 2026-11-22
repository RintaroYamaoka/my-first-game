import { readFileSync } from "node:fs";
import { z } from "zod";
import { parsePort } from "./parse";

const Config = z.object({
  port: z.number().int().min(1).max(65535),
  dataFile: z.string(),
  logFile: z.string().optional(),
});

export type Config = z.infer<typeof Config>;

export function load(path = "config.json"): Config {
  const raw = JSON.parse(readFileSync(path, "utf8"));
  raw.port = parsePort(raw.port);
  return Config.parse(raw);
}
