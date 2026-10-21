//! Seeded multi-device simulation with injected faults, crashes and clock skew.
//!
//! Every scenario ends with faults off and a full sync of all devices; then every device's
//! canonical dump must equal the state computed directly from the remote segments, and no
//! local op may be left behind.
//!
//! `BOTHY_SIM_SEEDS=<n>` sets the seeds per test (default 50, 10 tests). `BOTHY_SIM_SEED=<s>`
//! replays one seed.
#![cfg(feature = "test-util")]

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicI64, AtomicU64, Ordering};

use bothy::codec::cbor::Encoder;
use bothy::codec::keys::{KeyKind, LogicalKey};
use bothy::codec::segment;
use bothy::codec::state::encode_entry;
use bothy::merge::DocState;
use bothy::remote::faulty::{Faults, FaultyRemote, is_crash};
use bothy::remote::{FolderRemote, RemoteStore};
use bothy::store::{PackLimits, Store};
use bothy::{Clock, CollectionName, Config, Database, DbId, DocId};
use serde_json::json;

const DEVICES: usize = 3;
const STEPS: usize = 80;
const T0: u64 = 1_700_000_000_000;

struct Rng(u64);

impl Rng {
    fn below(&mut self, n: u64) -> u64 {
        self.0 = self
            .0
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        (self.0 >> 33) % n
    }
}

/// Shared simulated time plus a per-device skew.
struct SimClock {
    base: Arc<AtomicU64>,
    skew: AtomicI64,
}

impl Clock for SimClock {
    fn now_ms(&self) -> u64 {
        self.base
            .load(Ordering::SeqCst)
            .saturating_add_signed(self.skew.load(Ordering::SeqCst))
    }
}

struct Device {
    path: PathBuf,
    clock: Arc<SimClock>,
    db: Option<Database>,
}

struct Sim {
    _dir: tempfile::TempDir,
    remote: Arc<FaultyRemote<FolderRemote>>,
    devices: Vec<Device>,
    rng: Rng,
    seed: u64,
    stats: Stats,
}

/// What happened during a run, to check that faults were really exercised.
#[derive(Default, Debug, Clone, Copy)]
struct Stats {
    crashes: u64,
    failed_syncs: u64,
    ok_syncs: u64,
}

impl Sim {
    fn new(seed: u64) -> Self {
        let dir = tempfile::tempdir().unwrap();
        let folder = FolderRemote::new(dir.path().join("remote")).unwrap();
        let faults = Faults {
            offline: 0.08,
            rate_limited: 0.04,
            lost_response: 0.1,
            list_lag: 3,
        };
        let remote = Arc::new(FaultyRemote::new(folder, faults, seed));
        let base = Arc::new(AtomicU64::new(T0));
        let devices = (0..DEVICES)
            .map(|i| Device {
                path: dir.path().join(format!("d{i}.db")),
                clock: Arc::new(SimClock {
                    base: base.clone(),
                    skew: AtomicI64::new(0),
                }),
                db: None,
            })
            .collect();
        Self {
            _dir: dir,
            remote,
            devices,
            rng: Rng(seed ^ 0x5eed),
            seed,
            stats: Stats::default(),
        }
    }

    fn config(&self, i: usize) -> Config {
        // Small segments so packing splits often.
        let limits = PackLimits {
            max_ops: 4,
            max_plain_bytes: 300,
            compress: i.is_multiple_of(2),
        };
        Config::new(&self.devices[i].path, "com.example.sim")
            .clock(self.devices[i].clock.clone())
            .pack_limits(limits)
    }

    async fn db(&mut self, i: usize) -> Database {
        if self.devices[i].db.is_none() {
            let db = Database::open(self.config(i), self.remote.clone())
                .await
                .unwrap();
            self.devices[i].db = Some(db);
        }
        self.devices[i].db.clone().unwrap()
    }

    fn crash(&mut self, i: usize) {
        self.devices[i].db = None;
    }

    async fn setup(&mut self) -> DbId {
        self.remote.set_enabled(false);
        let db_id = self.db(0).await.init().await.unwrap();
        for i in 1..DEVICES {
            self.db(i).await.join(db_id).await.unwrap();
        }
        self.remote.set_enabled(true);
        db_id
    }

    async fn write(&mut self, i: usize) {
        let db = self.db(i).await;
        let items = db.collection("items").unwrap();
        let id = format!("d{}", self.rng.below(5));
        let field = ["a", "b", "c"][self.rng.below(3) as usize];
        let value = self.rng.below(1000) as i64;
        match self.rng.below(10) {
            0..=3 => {
                let other = ["a", "b", "c"][self.rng.below(3) as usize];
                items
                    .put(&id, json!({ field: value, other: -value }))
                    .await
                    .unwrap();
            }
            4..=6 => {
                items
                    .patch(&id, json!({ field: value }), &[])
                    .await
                    .unwrap();
            }
            7 => {
                items.patch(&id, json!({}), &[field]).await.unwrap();
            }
            _ => {
                items.delete(&id).await.unwrap();
            }
        };
    }

    async fn sync(&mut self, i: usize) {
        let db = self.db(i).await;
        match db.sync().await {
            Ok(_) => self.stats.ok_syncs += 1,
            Err(e) if is_crash(&e) => {
                self.stats.crashes += 1;
                self.crash(i);
            }
            Err(_) => self.stats.failed_syncs += 1,
        }
    }

    async fn run(&mut self) -> Stats {
        let db_id = self.setup().await;
        for _ in 0..STEPS {
            let i = self.rng.below(DEVICES as u64) as usize;
            match self.rng.below(100) {
                0..=49 => self.write(i).await,
                50..=79 => self.sync(i).await,
                80..=89 => {
                    self.remote.crash_after(self.rng.below(4) as u32);
                    self.sync(i).await;
                    self.remote.disarm();
                }
                90..=94 => {
                    let ms = self.rng.below(5_000);
                    self.devices[0].clock.base.fetch_add(ms, Ordering::SeqCst);
                }
                _ => {
                    // Clock skew: up to 10 minutes ahead or 10 seconds behind.
                    let skew = if self.rng.below(2) == 0 {
                        self.rng.below(600_000) as i64
                    } else {
                        -(self.rng.below(10_000) as i64)
                    };
                    self.devices[i].clock.skew.store(skew, Ordering::SeqCst);
                }
            }
        }
        self.finish(db_id).await;
        self.stats
    }

    async fn finish(&mut self, db_id: DbId) {
        self.remote.set_enabled(false);
        for _ in 0..3 {
            for i in 0..DEVICES {
                self.db(i)
                    .await
                    .sync()
                    .await
                    .unwrap_or_else(|e| panic!("seed {}: {e}", self.seed));
            }
        }
        let expected = oracle(self.remote.inner(), db_id).await;
        for i in 0..DEVICES {
            let dump = self.db(i).await.canonical_dump().await.unwrap();
            assert_eq!(
                dump, expected,
                "seed {}: device {i} differs from the remote state",
                self.seed
            );
        }
        // No local op was left behind: everything in the outbox reached the remote.
        for i in 0..DEVICES {
            self.crash(i);
            let store = Store::open(&self.devices[i].path, self.devices[i].clock.clone()).unwrap();
            assert!(
                store.unpacked_ops().unwrap().is_empty(),
                "seed {}: unpacked ops",
                self.seed
            );
            assert!(
                store.pending_uploads().unwrap().is_empty(),
                "seed {}: pending uploads",
                self.seed
            );
            let remote_ops = remote_op_count(self.remote.inner(), db_id, store.device_id()).await;
            assert_eq!(
                store.outbox_count().unwrap(),
                remote_ops,
                "seed {}: device {i} lost ops",
                self.seed
            );
        }
    }
}

/// State computed directly from every segment in the remote.
async fn oracle(remote: &FolderRemote, db_id: DbId) -> Vec<u8> {
    let mut states: BTreeMap<(CollectionName, DocId), DocState> = BTreeMap::new();
    for e in remote.list(&format!("{db_id}/")).await.unwrap() {
        let Some(LogicalKey {
            kind: KeyKind::Segment(dev, seq),
            ..
        }) = LogicalKey::parse(&e.key)
        else {
            continue;
        };
        let bytes = remote.get(&e.key).await.unwrap().unwrap();
        let seg = segment::decode(
            &bytes,
            &segment::Expected {
                db_id,
                device_id: dev,
                seq,
            },
        )
        .unwrap();
        for op in seg.ops {
            states
                .entry((op.collection.clone(), op.doc_id.clone()))
                .or_default()
                .apply(&op.kind, op.stamp(dev));
        }
    }
    let mut enc = Encoder::new();
    enc.array(states.len());
    for ((c, d), s) in &states {
        encode_entry(&mut enc, c, d, s).unwrap();
    }
    enc.into_bytes()
}

async fn remote_op_count(remote: &FolderRemote, db_id: DbId, device: bothy::DeviceId) -> u64 {
    let mut n = 0;
    for e in remote
        .list(&format!("{db_id}/devices/{device}/"))
        .await
        .unwrap()
    {
        if let Some(LogicalKey {
            kind: KeyKind::Segment(_, seq),
            ..
        }) = LogicalKey::parse(&e.key)
        {
            let bytes = remote.get(&e.key).await.unwrap().unwrap();
            let exp = segment::Expected {
                db_id,
                device_id: device,
                seq,
            };
            n += segment::decode(&bytes, &exp).unwrap().ops.len() as u64;
        }
    }
    n
}

fn seeds_per_test() -> u64 {
    std::env::var("BOTHY_SIM_SEEDS")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(50)
}

async fn run_range(block: u64) {
    if let Some(seed) = std::env::var("BOTHY_SIM_SEED")
        .ok()
        .and_then(|s| s.parse().ok())
    {
        if block == 0 {
            Sim::new(seed).run().await;
        }
        return;
    }
    let n = seeds_per_test();
    let mut total = Stats::default();
    for seed in block * n..(block + 1) * n {
        let s = Sim::new(seed).run().await;
        total.crashes += s.crashes;
        total.failed_syncs += s.failed_syncs;
        total.ok_syncs += s.ok_syncs;
    }
    eprintln!("block {block}: {total:?}");
    if n >= 10 {
        assert!(
            total.crashes > 0 && total.failed_syncs > 0 && total.ok_syncs > 0,
            "faults not exercised: {total:?}"
        );
    }
}

macro_rules! sim_blocks {
    ($($name:ident = $block:expr),* $(,)?) => {
        $(
            #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
            async fn $name() {
                run_range($block).await;
            }
        )*
    };
}

sim_blocks!(
    seeds_block_0 = 0,
    seeds_block_1 = 1,
    seeds_block_2 = 2,
    seeds_block_3 = 3,
    seeds_block_4 = 4,
    seeds_block_5 = 5,
    seeds_block_6 = 6,
    seeds_block_7 = 7,
    seeds_block_8 = 8,
    seeds_block_9 = 9,
);
