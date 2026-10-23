"""The agent against a local folder remote: a "phone" writes missions, the agent runs them and
reports telemetry, including 48 hours written offline."""

import json
import os
import shutil
import subprocess
import sys
import tempfile
import unittest

import bothy

HERE = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
AGENT = os.path.join(HERE, "agent.py")
APP_ID = "dev.bothy.notes"


class AgentTest(unittest.TestCase):
    def setUp(self):
        self.dir = tempfile.mkdtemp(prefix="bothy-agent-")
        self.remote = os.path.join(self.dir, "remote")
        self.home = os.path.join(self.dir, "home")

    def tearDown(self):
        shutil.rmtree(self.dir, ignore_errors=True)

    def agent(self, *args, password=None):
        env = dict(os.environ)
        env.pop("BOTHY_PASSWORD", None)
        if password:
            env["BOTHY_PASSWORD"] = password
        out = subprocess.run([sys.executable, AGENT, "--home", self.home, *args,
                              "--remote-dir", self.remote, "--name", "test-robot"],
                             env=env, check=True, capture_output=True, text=True, timeout=300)
        return [json.loads(line) for line in out.stdout.splitlines() if line.startswith("{")]

    def phone(self):
        return bothy.Database.open(os.path.join(self.dir, "phone.db"), APP_ID,
                                   bothy.FolderRemote(self.remote), password="pw", encrypt=True)

    def test_missions_offline_hours_and_memory(self):
        with self.phone() as phone:
            phone.init_or_join()
            phone.collection("missions").put("m1", {"goal": "dock", "state": "new"})
            phone.sync()

        events = self.agent("run", "--once", password="pw")
        self.assertIn("joined", [e["event"] for e in events])
        self.assertTrue(os.path.exists(os.path.join(self.home, "key")))
        self.assertEqual(oct(os.stat(os.path.join(self.home, "key")).st_mode & 0o777), "0o600")

        # 48 hours offline, then a normal run that syncs (no password: the kept key).
        offline = self.agent("run", "--once", "--offline", "--simulate-hours", "48")
        self.assertEqual([e["records"] for e in offline if e["event"] == "simulated"], [2880])
        status = json.loads(subprocess.run(
            [sys.executable, AGENT, "--home", self.home, "status", "--remote-dir", self.remote],
            check=True, capture_output=True, text=True).stdout)
        self.assertEqual(status["telemetry"], 2 + 2880)
        self.assertGreater(status["unsynced_ops"], 0)
        done = self.agent("run", "--once")
        peak = max(e.get("peak_rss_kib", 0) for e in offline + done)
        print(f"agent peak RSS: {peak} KiB", file=sys.stderr)

        with self.phone() as phone:
            phone.sync()
            m1 = phone.collection("missions").get("m1")
            self.assertEqual((m1["state"], m1["result"]), ("done", "ok"))
            logs = list(phone.collection("telemetry_logs").all())
            self.assertEqual(len(logs), 3 + 2880)
            self.assertEqual(sum(1 for _, r in logs if r.get("simulated")), 2880)


if __name__ == "__main__":
    unittest.main()
