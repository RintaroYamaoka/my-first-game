import { backup } from "./backup";
import { log } from "./log";
import { start } from "./server";

// 起動の見本: 関数の外に書かれた処理は、このファイルが読み込まれたときに、上から一度だけ走る
const INTERVAL = 60_000;
let runs = 0;

log("boot");
const server = start();

setInterval(() => {
  runs += 1;
  backup();
}, INTERVAL);

process.on("SIGINT", () => {
  log(`stop after ${runs} backups`);
  server.close();
});
