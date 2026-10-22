#!/usr/bin/env python3
"""Measures Bothy on this board: unlock time with the default Argon2id cost, and peak memory
when writing and syncing a day or two of telemetry with a kept key.

    python3 measure.py [--records 2880]

Each phase runs in its own process so its peak memory is its own. Everything happens in a
temporary directory that is removed at the end.
"""

import argparse
import json
import os
import resource
import shutil
import subprocess
import sys
import tempfile
import time

import bothy

APP = "dev.bothy.measure"


def peak_rss_kib():
    peak = resource.getrusage(resource.RUSAGE_SELF).ru_maxrss
    return peak // 1024 if sys.platform == "darwin" else peak


def phase(name, d, records):
    remote = bothy.FolderRemote(os.path.join(d, "remote"))
    path = os.path.join(d, "board.db")
    out = {"phase": name}
    if name == "create":
        t = time.perf_counter()
        db = bothy.Database.open(path, APP, remote, encrypt=True, password="measure-pw")
        db.init()
        out["create_s"] = round(time.perf_counter() - t, 3)
        with open(os.path.join(d, "key"), "wb") as f:
            f.write(db.export_key())
        db.close()
    elif name == "unlock":
        t = time.perf_counter()
        db = bothy.Database.open(path, APP, remote, password="measure-pw")
        out["unlock_s"] = round(time.perf_counter() - t, 3)
        out["locked"] = db.is_locked()
        db.close()
    elif name == "sync":
        with open(os.path.join(d, "key"), "rb") as f:
            key = f.read()
        db = bothy.Database.open(path, APP, remote, key=key)
        logs = db.collection("telemetry_logs")
        t = time.perf_counter()
        start = int(time.time() * 1000) - records * 60_000
        for i in range(records):
            logs.put(f"t{i:06d}", {"at_ms": start + i * 60_000, "cpu_temp_c": 40.0 + i % 7,
                                   "load1": 0.25, "mem_available_mib": 512, "simulated": True})
        out["write_s"] = round(time.perf_counter() - t, 3)
        t = time.perf_counter()
        report = db.sync()
        out["sync_s"] = round(time.perf_counter() - t, 3)
        out["pushed_ops"] = report["pushed_ops"]
        out["segments_up"] = report["segments_up"]
        db.close()
    out["peak_rss_kib"] = peak_rss_kib()
    print(json.dumps(out), flush=True)


def main():
    p = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    p.add_argument("--records", type=int, default=2880, help="telemetry records (2880 = 48 h)")
    p.add_argument("--phase", help=argparse.SUPPRESS)
    p.add_argument("--dir", help=argparse.SUPPRESS)
    args = p.parse_args()
    if args.phase:
        phase(args.phase, args.dir, args.records)
        return
    d = tempfile.mkdtemp(prefix="bothy-measure-")
    try:
        baseline = peak_rss_kib()
        print(json.dumps({"python": sys.version.split()[0], "bothy": bothy.__version__,
                          "machine": os.uname().machine, "interpreter_rss_kib": baseline}))
        for name in ("create", "unlock", "sync"):
            subprocess.run([sys.executable, __file__, "--phase", name, "--dir", d,
                            "--records", str(args.records)], check=True)
    finally:
        shutil.rmtree(d, ignore_errors=True)


if __name__ == "__main__":
    main()
