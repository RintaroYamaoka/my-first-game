#!/usr/bin/env python3
"""中身の置き場所(メモリ / ディスク)を比べる。先に DISK=1 を付けて build.sh で組み立てておく。
  ./experiments/06-linux/compare.py            # kvm(WSL の上の Docker の中の QEMU)
出すもの: QEMU を起動してからシェルが出るまでの時間、QEMU が使ったメモリの最大、Linux の中での速さ"""
import os, re, subprocess, sys, time
root = os.path.abspath(os.path.join(os.path.dirname(__file__), "../.."))
accel = sys.argv[1] if len(sys.argv) > 1 else "kvm"
cpu = "-accel kvm -cpu host" if accel == "kvm" else "-accel tcg,thread=multi -cpu max"
for tag, mem in (("", 1024), ("-rust", 3072), ("-layers", 2048)):
    for place in ("memory", "disk"):
        d = f"{root}/target/linux-image{tag}" + ("-disk" if place == "disk" else "")
        if not os.path.exists(d + "/initramfs.gz"):
            continue
        drive = "-drive file=/d/root.squashfs,format=raw,if=virtio,readonly=on" if place == "disk" else ""
        q = f"qemu-system-x86_64 {cpu} -m {mem} -smp 2 -nographic -no-reboot -kernel /d/vmlinuz -initrd /d/initramfs.gz {drive} -append 'console=ttyS0 quiet bench=5'"
        # QEMU が使ったメモリの最大(VmHWM)を、動いている間じゅう読んで、最後に出す
        sh = f"{q} & p=$!; while kill -0 $p 2>/dev/null; do v=$(grep VmHWM /proc/$p/status 2>/dev/null) && echo $v > /tmp/hwm; sleep 0.1; done; echo; cat /tmp/hwm"
        dev = ["--device", "/dev/kvm"] if accel == "kvm" else []
        t0 = time.time()
        p = subprocess.Popen(["docker", "run", "--rm", *dev, "-v", f"{d}:/d:ro", "my-first-game-linux:bench", "sh", "-c", sh], stdout=subprocess.PIPE, stderr=subprocess.STDOUT, text=True, errors="replace")
        ready, lines = None, []
        for line in p.stdout:
            line = re.sub(r"\x1b\[[0-9;?]*[a-zA-Z]|\x1bc|\r", "", line).rstrip()
            if "[boot]" in line and ready is None:
                ready = time.time() - t0
            if ready is not None and line and "reboot:" not in line:
                lines.append(line)
        p.wait()
        size = sum(os.path.getsize(f"{d}/{f}") for f in os.listdir(d)) / 1e6
        print(f"== 中身{tag or '(いつもの)'} / {place} / 割り当て {mem} MB / ファイル {size:.0f} MB ==")
        print(f"QEMU の起動からシェルまで           {ready or -1:6.2f} s")
        for l in lines:
            m = re.match(r"VmHWM:\s+(\d+) kB", l)
            print(f"QEMU が使ったメモリの最大          {int(m.group(1)) // 1024:5d} MB" if m else l)
        sys.stdout.flush()
