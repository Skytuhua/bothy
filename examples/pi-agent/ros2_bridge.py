"""ROS 2 bridge for the Bothy agent (``agent.py run --ros2``).

Publishes each accepted mission as JSON on ``/bothy/missions`` (``std_msgs/String``) and writes
every JSON message received on ``/bothy/telemetry`` to the ``telemetry_logs`` collection, so a
ROS 2 robot stack can take missions from the phone and report back. Needs a sourced ROS 2
environment (``source /opt/ros/<distro>/setup.bash``); nothing to build.

    ros2 topic echo /bothy/missions
    ros2 topic pub --once /bothy/telemetry std_msgs/String "{data: '{\\"battery\\": 0.8}'}"
"""

import json
import time

import rclpy
from rclpy.node import Node
from std_msgs.msg import String


class Bridge:
    def __init__(self, db):
        self.db = db
        rclpy.init()
        self.node = Node("bothy_bridge")
        self.missions = self.node.create_publisher(String, "/bothy/missions", 10)
        self.node.create_subscription(String, "/bothy/telemetry", self._telemetry, 10)
        self.node.get_logger().info("bridging /bothy/missions and /bothy/telemetry")

    def publish_mission(self, mission_id, mission):
        self.missions.publish(String(data=json.dumps({"id": mission_id, **mission})))

    def _telemetry(self, msg):
        try:
            rec = json.loads(msg.data)
        except ValueError:
            self.node.get_logger().warning("ignored a /bothy/telemetry message that is not JSON")
            return
        if not isinstance(rec, dict):
            rec = {"value": rec}
        now = int(time.time() * 1000)
        rec.setdefault("at_ms", now)
        rec["source"] = "ros2"
        self.db.collection("telemetry_logs").put(f"ros-{self.db.device_id[:8]}-{now}", rec)

    def spin_once(self, timeout):
        rclpy.spin_once(self.node, timeout_sec=timeout)

    def close(self):
        self.node.destroy_node()
        rclpy.shutdown()
