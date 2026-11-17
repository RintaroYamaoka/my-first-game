// 起動したままの Node.js。頼まれたファイルを実行する(Node.js の起動の時間をなくせるかを測るため)
//   "import <ファイル>" … 同じプロセスの中で読み込む / "worker <ファイル>" … 別のスレッドで実行する(毎回まっさら)
import net from "node:net";
import { Worker } from "node:worker_threads";
import { pathToFileURL } from "node:url";
let n = 0;
net.createServer((c) => {
  c.once("data", async (d) => {
    const [mode, file] = String(d).trim().split(" ");
    try {
      if (mode === "import") await import(pathToFileURL(file).href + "?" + n++);
      else await new Promise((ok, ng) => new Worker(file).on("exit", ok).on("error", ng));
      c.end("ok\n");
    } catch (e) {
      c.end("error " + e.message + "\n");
    }
  });
}).listen(7000, "127.0.0.1", () => console.log("ready"));
