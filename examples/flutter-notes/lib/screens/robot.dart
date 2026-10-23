import 'dart:async';

import 'package:bothy/bothy.dart';
import 'package:flutter/material.dart';

/// Missions for a robot agent (`examples/pi-agent`) and the telemetry it sends back.
///
/// A mission is `{goal, state, created_ms}`. The agent marks it `accepted` and then `done`.
/// Telemetry records are `{device, at_ms, cpu_temp_c, load1, mem_available_mib, ...}`.
class RobotScreen extends StatefulWidget {
  const RobotScreen({super.key, required this.db, required this.onSync});

  final Bothy db;
  final Future<void> Function() onSync;

  @override
  State<RobotScreen> createState() => _RobotScreenState();
}

class _RobotScreenState extends State<RobotScreen> {
  late final _missions = widget.db.collection('missions');
  late final _telemetry = widget.db.collection('telemetry_logs');
  StreamSubscription<BothyChange>? _changes;
  List<BothyDocument> _missionRows = [];
  List<BothyDocument> _latest = [];
  int _telemetryCount = 0;

  @override
  void initState() {
    super.initState();
    _changes = widget.db.watch().listen((c) {
      if (c.collection == 'missions' || c.collection == 'telemetry_logs') {
        _load();
      }
    });
    _load();
  }

  @override
  void dispose() {
    _changes?.cancel();
    super.dispose();
  }

  static int _ms(BothyDocument d, String field) =>
      (d.data[field] as num?)?.toInt() ?? 0;

  Future<List<BothyDocument>> _all(BothyCollection c) async {
    final out = <BothyDocument>[];
    String? after;
    while (true) {
      final page = await c.list(limit: 1000, after: after);
      out.addAll(page);
      if (page.length < 1000) return out;
      after = page.last.id;
    }
  }

  Future<void> _load() async {
    final missions = await _all(_missions);
    missions.sort(
      (a, b) => _ms(b, 'created_ms').compareTo(_ms(a, 'created_ms')),
    );
    final telemetry = await _all(_telemetry);
    telemetry.sort((a, b) => _ms(b, 'at_ms').compareTo(_ms(a, 'at_ms')));
    if (mounted) {
      setState(() {
        _missionRows = missions;
        _telemetryCount = telemetry.length;
        _latest = telemetry.take(20).toList();
      });
    }
  }

  Future<void> _newMission() async {
    final goal = TextEditingController();
    final ok = await showDialog<bool>(
      context: context,
      builder: (context) => AlertDialog(
        title: const Text('New mission'),
        content: TextField(
          controller: goal,
          autofocus: true,
          decoration: const InputDecoration(labelText: 'Goal (e.g. dock)'),
          onSubmitted: (_) => Navigator.pop(context, true),
        ),
        actions: [
          TextButton(
            onPressed: () => Navigator.pop(context, false),
            child: const Text('Cancel'),
          ),
          FilledButton(
            onPressed: () => Navigator.pop(context, true),
            child: const Text('Send'),
          ),
        ],
      ),
    );
    if (ok != true || goal.text.trim().isEmpty) return;
    final now = DateTime.now().millisecondsSinceEpoch;
    await _missions.put('m$now', {
      'goal': goal.text.trim(),
      'state': 'new',
      'created_ms': now,
    });
    await _load();
    await widget.onSync();
  }

  static String _time(int ms) {
    final t = DateTime.fromMillisecondsSinceEpoch(ms);
    String two(int v) => v.toString().padLeft(2, '0');
    return '${t.month}/${t.day} ${two(t.hour)}:${two(t.minute)}';
  }

  Widget _stateChip(String state) {
    final scheme = Theme.of(context).colorScheme;
    final color = switch (state) {
      'done' => scheme.primaryContainer,
      'accepted' => scheme.tertiaryContainer,
      _ => scheme.surfaceContainerHighest,
    };
    return Chip(label: Text(state), backgroundColor: color);
  }

  @override
  Widget build(BuildContext context) => Scaffold(
    appBar: AppBar(
      title: const Text('Robot'),
      actions: [
        IconButton(
          tooltip: 'Sync now',
          onPressed: () async {
            await widget.onSync();
            await _load();
          },
          icon: const Icon(Icons.sync),
        ),
      ],
    ),
    body: ListView(
      children: [
        const ListTile(title: Text('Missions')),
        if (_missionRows.isEmpty)
          const Padding(
            padding: EdgeInsets.symmetric(horizontal: 16),
            child: Text('No missions yet. Send one with +.'),
          ),
        for (final m in _missionRows)
          ListTile(
            leading: const Icon(Icons.flag_outlined),
            title: Text((m.data['goal'] as String?) ?? m.id),
            subtitle: Text(_time(_ms(m, 'created_ms'))),
            trailing: _stateChip((m.data['state'] as String?) ?? 'new'),
          ),
        const Divider(),
        ListTile(
          title: const Text('Telemetry'),
          subtitle: Text('$_telemetryCount record(s); latest first'),
        ),
        for (final t in _latest)
          ListTile(
            dense: true,
            leading: Icon(
              t.data['simulated'] == true ? Icons.history : Icons.sensors,
            ),
            title: Text(
              [
                if (t.data['cpu_temp_c'] != null) '${t.data['cpu_temp_c']} °C',
                if (t.data['load1'] != null) 'load ${t.data['load1']}',
                if (t.data['mem_available_mib'] != null)
                  '${t.data['mem_available_mib']} MiB free',
                if (t.data['source'] == 'ros2') 'ROS 2',
              ].join(' · '),
            ),
            subtitle: Text(
              '${t.data['device'] ?? ''} · ${_time(_ms(t, 'at_ms'))}',
            ),
          ),
      ],
    ),
    floatingActionButton: FloatingActionButton(
      tooltip: 'New mission',
      onPressed: _newMission,
      child: const Icon(Icons.add),
    ),
  );
}
