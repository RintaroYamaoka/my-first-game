import http from "node:http";
import { load } from "./config";
import { log } from "./log";
import { parse } from "./parse";
import { find } from "./store";

export function handle(req: http.IncomingMessage, res: http.ServerResponse): void {
  const id = parse(req.url);
  if (!id) {
    res.writeHead(400);
    res.end();
    return;
  }
  const f = find(id);
  if (!f) {
    res.writeHead(404);
    res.end();
    return;
  }
  log(`get ${id}`);
  res.end(JSON.stringify(f));
}

export function start(): http.Server {
  const cfg = load();
  return http.createServer(handle).listen(cfg.port);
}
