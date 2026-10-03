# Suppressions and smell patterns

Scope: `crates/*/src/**/*.rs`.

For `.unwrap()` / `.expect(` / `let _ =`: excluded when inside a `#[cfg(test)]` module or `mod tests { ... }` block (brace-depth heuristic from that opener).

## armee-kinematics

### allow (0)

(none)

### cfg_attr_allow (0)

(none)

### todo_fixme (0)

(none)

### todo_macro (0)

(none)

### ignore (1)
- `crates/armee-kinematics/src/lib.rs:268: #[ignore = "marengo.urdf is placeholder until Brawner export; run when commissioning full humanoid"]`

### let_underscore (0)

(none)

### unwrap (0)

(none)

### expect (0)

(none)

## berthier

### allow (17)
- `crates/berthier/src/position_feedforward.rs:20: #[allow(clippy::too_many_arguments)]`
- `crates/berthier/src/friction.rs:59: #[allow(clippy::too_many_arguments)]`
- `crates/berthier/src/friction.rs:154: #[allow(clippy::too_many_arguments)]`
- `crates/berthier/src/friction.rs:203: #[allow(dead_code)] // unit tests in this crate`
- `crates/berthier/src/position_trajectory.rs:180: #[allow(dead_code)] // unit tests in this crate`
- `crates/berthier/src/gain_runtime.rs:390: #[allow(clippy::unwrap_used)]`
- `crates/berthier/src/position_wave.rs:88: #[allow(clippy::unwrap_used)]`
- `crates/berthier/src/position_hold.rs:388: #[allow(dead_code)] // public status accessor; ControlLoop may wire later`
- `crates/berthier/src/position_hold.rs:1398: #[allow(clippy::unwrap_used)]`
- `crates/berthier/src/position_setpoint.rs:185: #[allow(dead_code)]`
- `crates/berthier/src/position_setpoint.rs:205: #[allow(dead_code)]`
- `crates/berthier/src/position_setpoint.rs:218: #[allow(dead_code, clippy::too_many_arguments)]`
- `crates/berthier/src/position_setpoint.rs:233: #[allow(dead_code)]`
- `crates/berthier/src/position_setpoint.rs:343: #[allow(clippy::too_many_arguments)]`
- `crates/berthier/src/mit_feedforward.rs:78: #[allow(clippy::unwrap_used)]`
- `crates/berthier/src/position_trace.rs:60: #[allow(dead_code)]`
- `crates/berthier/src/torque_cmd.rs:49: #[allow(clippy::unwrap_used)]`

### cfg_attr_allow (0)

(none)

### todo_fixme (0)

(none)

### todo_macro (0)

(none)

### ignore (0)

(none)

### let_underscore (1)
- `crates/berthier/src/loop.rs:1337: let _ = trace.maybe_record(self.tick_count, t_ms, &row);`

### unwrap (0)

(none)

### expect (56)
- `crates/berthier/src/reference_journal_tests.rs:56: .expect("matching explicit factory");`
- `crates/berthier/src/reference_journal_tests.rs:58: .expect("old controller intent without drive permission");`
- `crates/berthier/src/reference_journal_tests.rs:80: .expect("Enable raw reply");`
- `crates/berthier/src/reference_journal_tests.rs:90: .expect("real SetZero/pop rule");`
- `crates/berthier/src/reference_journal_tests.rs:95: .expect("actual stamp");`
- `crates/berthier/src/reference_journal_tests.rs:104: .expect("actual reservation");`
- `crates/berthier/src/reference_journal_tests.rs:106: ctrl.tick(None).expect("actual tick owns each phase");`
- `crates/berthier/src/reference_journal_tests.rs:112: .expect("actual terminal")`
- `crates/berthier/src/reference_journal_tests.rs:127: .expect("actual journal job");`
- `crates/berthier/src/reference_journal_tests.rs:136: let text = std::fs::read_to_string(&path).expect("copied policy");`
- `crates/berthier/src/reference_journal_tests.rs:144: .expect("isolated diagnostics");`
- `crates/berthier/src/reference_journal_tests.rs:152: .expect("real SQL/readback gate");`
- `crates/berthier/src/reference_journal_tests.rs:165: .expect("ordered actual raw queue");`
- `crates/berthier/src/reference_journal_tests.rs:168: .expect("owner retains whole fault outcome without duplicate runtime fallback");`
- `crates/berthier/src/reference_journal_tests.rs:172: .expect("retained commit");`
- `crates/berthier/src/reference_journal_tests.rs:173: let receive = snapshot.receive.expect("actual sole tick report");`
- `crates/berthier/src/reference_journal_tests.rs:195: ctrl.tick(None).expect("one owner advance");`
- `crates/berthier/src/reference_journal_tests.rs:202: .expect("actual completion retained");`
- `crates/berthier/src/reference_journal_tests.rs:214: let pause = JournalTestPause::new(JournalPausePoint::BeforePublication, 1).expect("gate");`
- `crates/berthier/src/reference_journal_tests.rs:219: .expect("cancel eligibility only");`
- `crates/berthier/src/reference_journal_tests.rs:221: .expect("ordinary disabled intent remains separate from output permission");`
- `crates/berthier/src/reference_journal_tests.rs:225: .expect("pending disk work still belongs to owner tick");`
- `crates/berthier/src/reference_journal_tests.rs:232: ctrl.tick(None).expect("real completion advance");`
- `crates/berthier/src/reference_journal_tests.rs:239: .expect("late retained disk outcome")`
- `crates/berthier/src/reference_grant_tests.rs:44: .expect("closed test cleanup");`
- `crates/berthier/src/reference_grant_tests.rs:63: let text = std::fs::read_to_string(&path).expect("owned policy");`
- `crates/berthier/src/reference_grant_tests.rs:71: .expect("isolated policy");`
- `crates/berthier/src/reference_grant_tests.rs:87: .expect("closed Unreferenced consuming controller");`
- `crates/berthier/src/reference_grant_tests.rs:93: .expect("old disabled intent");`
- `crates/berthier/src/reference_grant_tests.rs:107: .expect("literal Enable reply");`
- `crates/berthier/src/reference_grant_tests.rs:118: .expect("real SetZero/pop rule");`
- `crates/berthier/src/reference_grant_tests.rs:123: .expect("fresh stamp");`
- `crates/berthier/src/reference_grant_tests.rs:132: .expect("real acquisition");`
- `crates/berthier/src/reference_grant_tests.rs:134: ctrl.tick(None).expect("real owner phase tick");`
- `crates/berthier/src/reference_grant_tests.rs:140: .expect("actual terminal");`
- `crates/berthier/src/reference_grant_tests.rs:161: .expect("accepted real worker job");`
- `crates/berthier/src/reference_grant_tests.rs:169: .expect("real completion queue gate");`
- `crates/berthier/src/reference_grant_tests.rs:176: .expect("one fresh bounded report");`
- `crates/berthier/src/reference_grant_tests.rs:177: ctrl.tick(None).expect("sole consumer tick");`
- `crates/berthier/src/reference_grant_tests.rs:181: .expect("actual selected commit");`
- `crates/berthier/src/reference_grant_tests.rs:187: let receive = snapshot.receive.expect("actual report");`
- `crates/berthier/src/reference_grant_tests.rs:204: .expect("new selected Enable");`
- `crates/berthier/src/reference_grant_tests.rs:206: .expect("new explicit torque intent");`
- `crates/berthier/src/reference_grant_tests.rs:209: ctrl.tick(None).expect("real selected output tick");`
- `crates/berthier/src/reference_grant_tests.rs:228: .expect("actual queued completion");`
- `crates/berthier/src/reference_grant_tests.rs:243: .expect("whole raw report");`
- `crates/berthier/src/reference_grant_tests.rs:246: .expect("actual owner tick preserves hazard priority");`
- `crates/berthier/src/reference_grant_tests.rs:250: .expect("actual retained result");`
- `crates/berthier/src/reference_grant_tests.rs:260: let receive = snapshot.receive.expect("sole report");`
- `crates/berthier/src/reference_grant_tests.rs:284: .expect("actual completion gate");`
- `crates/berthier/src/reference_grant_tests.rs:287: ctrl.tick(None).expect("actual durable grant consumption");`
- `crates/berthier/src/reference_grant_tests.rs:291: .expect("current selected job")`
- `crates/berthier/src/reference_grant_tests.rs:297: .expect("real selected Enable");`
- `crates/berthier/src/reference_grant_tests.rs:300: .expect("intact selected Active shortcut");`
- `crates/berthier/src/reference_grant_tests.rs:305: .expect("revoke the actual owning job");`
- `crates/berthier/src/reference_grant_tests.rs:319: .expect("shared admission's complete stop report");`

## chappe

### allow (0)

(none)

### cfg_attr_allow (0)

(none)

### todo_fixme (0)

(none)

### todo_macro (0)

(none)

### ignore (0)

(none)

### let_underscore (13)
- `crates/chappe/src/tracing_layer.rs:199: let _ = self`
- `crates/chappe/src/ipc.rs:203: let _ = stream.shutdown(std::net::Shutdown::Both);`
- `crates/chappe/src/ipc.rs:267: let _ = bus.publish_bytes(&topic, payload);`
- `crates/chappe/src/ipc.rs:370: let _ = std::fs::remove_file(&socket_path);`
- `crates/chappe/src/ipc.rs:377: let _ = std::fs::set_permissions(`
- `crates/chappe/src/ipc.rs:424: let _ = writer.shutdown(std::net::Shutdown::Both);`
- `crates/chappe/src/ipc.rs:428: let _ = old.shutdown(std::net::Shutdown::Both);`
- `crates/chappe/src/ipc.rs:467: let _ = old.shutdown(std::net::Shutdown::Both);`
- `crates/chappe/src/ipc.rs:491: let _ = peer.shutdown(std::net::Shutdown::Both);`
- `crates/chappe/src/ipc.rs:528: let _ = s.shutdown(std::net::Shutdown::Both);`
- `crates/chappe/src/ipc.rs:591: let _ = direction;`
- `crates/chappe/src/ipc_outbox.rs:104: let _ = self.connection_changes.send(connected);`
- `crates/chappe/src/ipc_outbox.rs:113: let _ = self.connection_changes.send(false);`

### unwrap (0)

(none)

### expect (0)

(none)

## davout

### allow (1)
- `crates/davout/src/reference_journal.rs:1069: #[allow(`

### cfg_attr_allow (0)

(none)

### todo_fixme (0)

(none)

### todo_macro (0)

(none)

### ignore (0)

(none)

### let_underscore (6)
- `crates/davout/src/reference_commit.rs:391: let _ = self.perform_stop(false);`
- `crates/davout/src/reference_transaction.rs:1444: let _ =`
- `crates/davout/src/reference_transaction.rs:1447: let _ = self.perform_stop(false);`
- `crates/davout/src/reference_transaction.rs:1567: let _ = self.observe_staged_reference();`
- `crates/davout/src/reference_journal_filetype_tests.rs:15: let _ = child.kill();`
- `crates/davout/src/reference_journal_filetype_tests.rs:16: let _ = child.wait();`

### unwrap (0)

(none)

### expect (239)
- `crates/davout/src/reference_journal_tests.rs:28: let text = std::fs::read_to_string(&path).expect("copied control config");`
- `crates/davout/src/reference_journal_tests.rs:40: .expect("isolated diagnostics");`
- `crates/davout/src/reference_journal_tests.rs:100: .expect("explicit unreferenced journal owner");`
- `crates/davout/src/reference_journal_tests.rs:117: .expect("Enable status input");`
- `crates/davout/src/reference_journal_tests.rs:130: .expect("real SetZero/pop correlation");`
- `crates/davout/src/reference_journal_tests.rs:136: .expect("fresh owner stamp");`
- `crates/davout/src/reference_journal_tests.rs:144: .expect("actual acquisition");`
- `crates/davout/src/reference_journal_tests.rs:148: .expect("actual owner phase");`
- `crates/davout/src/reference_journal_tests.rs:151: let terminal = snapshot.terminal.expect("actual terminal");`
- `crates/davout/src/reference_journal_tests.rs:175: .expect("actual retained stage")`
- `crates/davout/src/reference_journal_tests.rs:177: .expect("actual immutable input")`
- `crates/davout/src/reference_journal_tests.rs:187: .expect("actual ordered completion advance");`
- `crates/davout/src/reference_journal_tests.rs:213: .expect("retained actual stage")`
- `crates/davout/src/reference_journal_tests.rs:215: .expect("sealed worker input");`
- `crates/davout/src/reference_journal_tests.rs:217: crate::reference_codec::decode(&input.encode(1, 1).expect("typed body"))`
- `crates/davout/src/reference_journal_tests.rs:218: .expect("canonical typed decode");`
- `crates/davout/src/reference_journal_tests.rs:226: .expect("one accepted job");`
- `crates/davout/src/reference_journal_tests.rs:230: .expect("identical retry"),`
- `crates/davout/src/reference_journal_tests.rs:259: .expect("reopen actual committed SQL");`
- `crates/davout/src/reference_journal_tests.rs:276: .expect("new owner's actual job");`
- `crates/davout/src/reference_journal_tests.rs:292: JournalTestPause::new(JournalPausePoint::BeforePublication, 1).expect("declarative pause");`
- `crates/davout/src/reference_journal_tests.rs:297: .expect("accepted job");`
- `crates/davout/src/reference_journal_tests.rs:302: .expect("cancel eligibility only");`
- `crates/davout/src/reference_journal_tests.rs:328: let pause = JournalTestPause::new(JournalPausePoint::BeforePublication, 1).expect("pause");`
- `crates/davout/src/reference_journal_tests.rs:333: .expect("accepted");`
- `crates/davout/src/reference_journal_tests.rs:338: .expect("exact original deadline");`
- `crates/davout/src/reference_journal_tests.rs:358: std::fs::create_dir(&path).expect("blocked file resource");`
- `crates/davout/src/reference_journal_tests.rs:360: std::fs::write(&path, b"preserve-this-corrupt-fixture").expect("owned malformed data");`
- `crates/davout/src/reference_journal_tests.rs:366: .expect("worker admission precedes filesystem validation");`
- `crates/davout/src/reference_journal_tests.rs:375: std::fs::read(&path).expect("preserved file"),`
- `crates/davout/src/reference_journal_tests.rs:391: .expect("first event");`
- `crates/davout/src/reference_journal_tests.rs:397: let mut database = Connection::open(&path).expect("real competing connection");`
- `crates/davout/src/reference_journal_tests.rs:400: .expect("actual exclusive lock");`
- `crates/davout/src/reference_journal_tests.rs:405: .expect("admission");`
- `crates/davout/src/reference_journal_tests.rs:411: transaction.rollback().expect("release owned SQL lock");`
- `crates/davout/src/reference_journal_tests.rs:415: .expect("preserved prior event")`
- `crates/davout/src/reference_journal_tests.rs:430: .expect("ordinary virtual factory");`
- `crates/davout/src/reference_journal_tests.rs:454: let pause = JournalTestPause::new(JournalPausePoint::BeforeOpen, 1).expect("actual I/O gate");`
- `crates/davout/src/reference_journal_tests.rs:461: .expect("reserved completion credit");`
- `crates/davout/src/reference_journal_tests.rs:464: .expect("retire eligibility, retain disk work");`
- `crates/davout/src/reference_journal_tests.rs:477: .expect("pending correlations never evicted")`
- `crates/davout/src/reference_journal_tests.rs:493: .expect("identical rejected admission is retryable after credit return");`
- `crates/davout/src/reference_journal_tests.rs:518: Connection::open(tree.path().join("reference.sqlite3")).expect("actual reopened history");`
- `crates/davout/src/reference_journal_tests.rs:522: .expect("real rows"),`
- `crates/davout/src/reference_journal_tests.rs:532: .expect("after actual commit/readback");`
- `crates/davout/src/reference_journal_tests.rs:537: .expect("real job");`
- `crates/davout/src/reference_journal_tests.rs:550: .expect("literal ordered peer fault/noise");`
- `crates/davout/src/reference_journal_tests.rs:555: .expect("one bounded actual report");`
- `crates/davout/src/reference_journal_tests.rs:556: let receive = observed.receive.expect("whole ordered report");`
- `crates/davout/src/reference_journal_tests.rs:595: let pause = JournalTestPause::new(JournalPausePoint::BeforePublication, 1).expect("pause");`
- `crates/davout/src/reference_journal_tests.rs:601: .expect("real job");`
- `crates/davout/src/reference_journal_tests.rs:610: .expect("observed mismatch");`
- `crates/davout/src/reference_journal_tests.rs:630: .expect("fixed real worker unwind");`
- `crates/davout/src/reference_journal_tests.rs:635: .expect("real job");`
- `crates/davout/src/reference_journal_tests.rs:639: .expect("release eligibility only");`
- `crates/davout/src/reference_journal_tests.rs:643: .expect("queued accepted job");`
- `crates/davout/src/reference_journal_tests.rs:675: .expect("actual post-unwind commit recovery")`
- `crates/davout/src/reference_journal_tests.rs:763: .expect("typed immutable model install before acquisition");`
- `crates/davout/src/reference_journal_tests.rs:768: .expect("real typed capture");`
- `crates/davout/src/reference_journal_tests.rs:778: .expect("real owned decoded URDF");`
- `crates/davout/src/reference_journal_tests.rs:794: .expect("optional material")`
- `crates/davout/src/reference_journal_tests.rs:797: .expect("color")`
- `crates/davout/src/reference_journal_tests.rs:807: .expect("dynamics")`
- `crates/davout/src/reference_journal_tests.rs:816: .expect("mimic")`
- `crates/davout/src/reference_journal_tests.rs:818: .expect("offset")`
- `crates/davout/src/reference_journal_tests.rs:826: .expect("safety controller")`
- `crates/davout/src/reference_journal_tests.rs:851: .expect("real baseline event");`
- `crates/davout/src/reference_journal_tests.rs:858: .expect("owned fixture SQL connection")`
- `crates/davout/src/reference_journal_tests.rs:860: .expect("explicit fixture alteration");`
- `crates/davout/src/reference_journal_tests.rs:861: let original = std::fs::read(&path).expect("exact altered resource");`
- `crates/davout/src/reference_journal_tests.rs:866: .expect("worker admission");`
- `crates/davout/src/reference_journal_tests.rs:873: std::fs::read(&path).expect("preserved evidence"),`
- `crates/davout/src/reference_journal_tests.rs:892: let pause = JournalTestPause::new(point, 1).expect("exact transaction pause");`
- `crates/davout/src/reference_journal_tests.rs:900: .expect("controlled child owner");`
- `crates/davout/src/reference_journal_tests.rs:905: .expect("real child job");`
- `crates/davout/src/reference_journal_tests.rs:915: std::process::Command::new(std::env::current_exe().expect("actual test executable"));`
- `crates/davout/src/reference_journal_tests.rs:926: let status = child.status().expect("controlled child process");`
- `crates/davout/src/reference_journal_tests.rs:930: .expect("SQLite's own actual recovery");`
- `crates/davout/src/reference_journal_storage_tests.rs:22: let mut database = Database::open(&path, true, true).expect("concrete pinned connection");`
- `crates/davout/src/reference_journal_storage_tests.rs:26: .expect("full-width unsigned key body");`
- `crates/davout/src/reference_journal_storage_tests.rs:30: .expect("actual SQL transaction/readback");`
- `crates/davout/src/reference_journal_storage_tests.rs:41: .expect("equal SQL retry"),`
- `crates/davout/src/reference_journal_storage_tests.rs:47: .expect("different valid body");`
- `crates/davout/src/reference_journal_storage_tests.rs:57: .expect("real row count");`
- `crates/davout/src/reference_journal_storage_tests.rs:60: let records = inspect(&path, 8).expect("actual reopened unsigned history");`
- `crates/davout/src/reference_journal_storage_tests.rs:69: let mut database = Database::open(&path, true, true).expect("real bounded SQLite connection");`
- `crates/davout/src/reference_journal_storage_tests.rs:73: .expect("actual configured cap");`
- `crates/davout/src/reference_journal_storage_tests.rs:78: .expect("existing pages");`
- `crates/davout/src/reference_journal_storage_tests.rs:83: .expect("actual smaller fixture storage capacity");`
- `crates/davout/src/reference_journal_storage_tests.rs:87: .expect("real captured body");`
- `crates/davout/src/reference_journal_storage_tests.rs:101: .expect("post-failure readback"),`
- `crates/davout/src/reference_journal_storage_tests.rs:106: .expect("intact actual database")`
- `crates/davout/src/reference_grant_tests.rs:53: let text = std::fs::read_to_string(&path).expect("owned policy");`
- `crates/davout/src/reference_grant_tests.rs:65: .expect("isolated reporting policy");`
- `crates/davout/src/reference_grant_tests.rs:92: .expect("explicit current-consuming Unreferenced factory");`
- `crates/davout/src/reference_grant_tests.rs:113: .expect("literal Enable reply");`
- `crates/davout/src/reference_grant_tests.rs:123: .expect("real correlated SetZero/pop rule");`
- `crates/davout/src/reference_grant_tests.rs:134: let stamp = owner.reference_snapshot().next_stamp.expect("fresh stamp");`
- `crates/davout/src/reference_grant_tests.rs:142: .expect("real reservation");`
- `crates/davout/src/reference_grant_tests.rs:147: .expect("real phase advance");`
- `crates/davout/src/reference_grant_tests.rs:152: .expect("actual terminal");`
- `crates/davout/src/reference_grant_tests.rs:185: .expect("actual accepted immutable worker job")`
- `crates/davout/src/reference_grant_tests.rs:192: .expect("real consumption");`
- `crates/davout/src/reference_grant_tests.rs:205: .expect("actual worker")`
- `crates/davout/src/reference_grant_tests.rs:219: .expect("pending observation")`
- `crates/davout/src/reference_grant_tests.rs:255: .expect("real selected Enable");`
- `crates/davout/src/reference_grant_tests.rs:259: .expect("post-enable raw pose");`
- `crates/davout/src/reference_grant_tests.rs:260: owner.drain_feedback().expect("actual post-enable receive");`
- `crates/davout/src/reference_grant_tests.rs:264: .expect("selected nonzero output");`
- `crates/davout/src/reference_grant_tests.rs:286: .expect("actual committed/readback history");`
- `crates/davout/src/reference_grant_tests.rs:291: .expect("fresh consumption");`
- `crates/davout/src/reference_grant_tests.rs:293: assert_eq!(selected.receive.expect("fresh report").raw_frames, 0);`
- `crates/davout/src/reference_grant_tests.rs:303: .expect("same selected Active shortcut");`
- `crates/davout/src/reference_grant_tests.rs:318: .expect("old transaction deadline equality");`
- `crates/davout/src/reference_grant_tests.rs:322: .expect("current lifetime")`
- `crates/davout/src/reference_grant_tests.rs:327: owner.disable_all().expect("ordinary successful stop");`
- `crates/davout/src/reference_grant_tests.rs:343: .expect("actual grant");`
- `crates/davout/src/reference_grant_tests.rs:387: .expect("fresh whole-report stream");`
- `crates/davout/src/reference_grant_tests.rs:391: .expect("truthful durable result");`
- `crates/davout/src/reference_grant_tests.rs:401: let report = result.receive.expect("fresh bounded actual receive");`
- `crates/davout/src/reference_grant_tests.rs:422: .expect("exact captured deadline");`
- `crates/davout/src/reference_grant_tests.rs:425: .expect("late durable consumption");`
- `crates/davout/src/reference_grant_tests.rs:451: .expect("bounded interrupted reads");`
- `crates/davout/src/reference_grant_tests.rs:455: .expect("actual bounded consumer");`
- `crates/davout/src/reference_grant_tests.rs:456: let receive = snapshot.receive.expect("fresh read report");`
- `crates/davout/src/reference_grant_tests.rs:482: .expect("real committed/readback pause");`
- `crates/davout/src/reference_grant_tests.rs:493: .expect("explicit cancel");`
- `crates/davout/src/reference_grant_tests.rs:512: .expect("real incompatible resource");`
- `crates/davout/src/reference_grant_tests.rs:529: .expect("actual after-COMMIT unwind");`
- `crates/davout/src/reference_grant_tests.rs:583: .expect("last current stamp");`
- `crates/davout/src/reference_grant_tests.rs:597: .expect("intact final generation")`
- `crates/davout/src/reference_grant_tests.rs:618: .expect("modeled reset after real write/readback");`
- `crates/davout/src/reference_grant_tests.rs:624: .expect("sticky current observation");`
- `crates/davout/src/reference_grant_tests.rs:633: .expect("late truthful consumption");`
- `crates/davout/src/reference_grant_tests.rs:660: .expect("bound policy")`
- `crates/davout/src/reference_grant_tests.rs:665: .expect("real equal-value model installation"),`
- `crates/davout/src/reference_grant_tests.rs:673: .expect("restored diagnostics")`
- `crates/davout/src/reference_grant_tests.rs:692: .expect("bound motor")`
- `crates/davout/src/reference_grant_tests.rs:698: .expect("valid output cap")`
- `crates/davout/src/reference_grant_tests.rs:702: let wire = owner.bus().frames().last().expect("actual MIT");`
- `crates/davout/src/reference_grant_tests.rs:709: .expect("target motor")`
- `crates/davout/src/reference_grant_tests.rs:733: .expect("checked modeled closed-device reset");`
- `crates/davout/src/reference_grant_tests.rs:737: .expect("cancel selected job");`
- `crates/davout/src/reference_grant_tests.rs:742: .expect("actual current projection")`
- `crates/davout/src/reference_grant_tests.rs:761: .expect("prior selection")`
- `crates/davout/src/reference_grant_tests.rs:770: .expect("old job cannot supply new grant")`
- `crates/davout/src/reference_grant_tests.rs:790: .expect("real raw peer fault");`
- `crates/davout/src/reference_grant_tests.rs:807: .expect("one actual failed stop attempt");`
- `crates/davout/src/reference_grant_tests.rs:813: .expect("all-address report")`
- `crates/davout/src/reference_grant_tests.rs:823: .expect("revoked projection")`
- `crates/davout/src/reference_grant_tests.rs:840: .expect("actual selected job after owner shutdown")`
- `crates/davout/src/reference_commit_tests.rs:31: let text = std::fs::read_to_string(&control).expect("owned policy");`
- `crates/davout/src/reference_commit_tests.rs:39: .expect("isolated diagnostics policy");`
- `crates/davout/src/reference_commit_tests.rs:47: .expect("explicit unreferenced owner"),`
- `crates/davout/src/reference_commit_tests.rs:68: .expect("literal Enable status");`
- `crates/davout/src/reference_commit_tests.rs:82: .expect("closed actual SetZero/pop causality");`
- `crates/davout/src/reference_commit_tests.rs:87: .expect("current stamp");`
- `crates/davout/src/reference_commit_tests.rs:96: .expect("actual acquisition");`
- `crates/davout/src/reference_commit_tests.rs:101: .expect("actual owner advance");`
- `crates/davout/src/reference_commit_tests.rs:108: .expect("terminal")`
- `crates/davout/src/reference_commit_tests.rs:130: .expect("journal installed")`
- `crates/davout/src/reference_commit_tests.rs:140: .expect("accepted job");`
- `crates/davout/src/reference_commit_tests.rs:155: .expect("pending owner")`
- `crates/davout/src/reference_commit_tests.rs:173: .expect("literal fresh ordered peer fault/noise");`
- `crates/davout/src/reference_commit_tests.rs:178: .expect("real owner collection");`
- `crates/davout/src/reference_commit_tests.rs:187: let receive = result.receive.expect("actual fresh report");`
- `crates/davout/src/reference_commit_tests.rs:226: .expect("first actual job");`
- `crates/davout/src/reference_commit_tests.rs:237: .expect("actual neighboring raw input");`
- `crates/davout/src/reference_commit_tests.rs:250: .expect("healthy neighboring stage");`
- `crates/davout/src/reference_commit_tests.rs:255: .expect("genuine matching completion");`
- `crates/davout/src/reference_commit_tests.rs:257: assert_eq!(completed.receive.expect("own report").raw_frames, 1);`
- `crates/davout/src/reference_journal_resource_tests.rs:41: .expect("fixture name")`
- `crates/davout/src/reference_journal_resource_tests.rs:46: .expect("process cwd")`
- `crates/davout/src/reference_journal_resource_tests.rs:52: fs::create_dir(&sub).expect("owned alias traversal directory");`
- `crates/davout/src/reference_journal_resource_tests.rs:61: std::os::unix::fs::symlink(&sub, &link).expect("owned symlink alias");`
- `crates/davout/src/reference_journal_resource_tests.rs:88: .expect("distinct explicit resources");`
- `crates/davout/src/reference_journal_resource_tests.rs:107: let mut db = Database::open(&original, true, true).expect("actual journal creation");`
- `crates/davout/src/reference_journal_resource_tests.rs:108: let body = input.encode(db.session, 1).expect("actual acquired input");`
- `crates/davout/src/reference_journal_resource_tests.rs:114: .expect("real durable row");`
- `crates/davout/src/reference_journal_resource_tests.rs:116: let connection = Connection::open(&original).expect("owned corruption fixture");`
- `crates/davout/src/reference_journal_resource_tests.rs:119: .expect("actual corrupt checksum");`
- `crates/davout/src/reference_journal_resource_tests.rs:122: .expect("real persistent WAL format");`
- `crates/davout/src/reference_journal_resource_tests.rs:125: let bytes = fs::read(&original).expect("closed incompatible bytes");`
- `crates/davout/src/reference_journal_resource_tests.rs:134: fs::write(&path, &bytes).expect("independent incompatible fixture");`
- `crates/davout/src/reference_journal_resource_tests.rs:140: let mut journal = Journal::spawn(path.clone(), None).expect("actual lazy worker");`
- `crates/davout/src/reference_journal_resource_tests.rs:146: .expect("accepted actual input");`
- `crates/davout/src/reference_journal_resource_tests.rs:148: let completion = journal.take_matching(&identity).expect("actual completion");`
- `crates/davout/src/reference_journal_resource_tests.rs:161: fs::read(&path).expect("preserved file") == bytes,`
- `crates/davout/src/active_reporting_pacing_tests.rs:69: let header = unpack_ext_id(frame.id).expect("encoded reporting ID");`
- `crates/davout/src/active_reporting_pacing_tests.rs:173: let header = unpack_ext_id(frame.id).expect("encoded reporting ID");`
- `crates/davout/src/reference_journal_namespace_tests.rs:19: fs::write(&history, bytes).expect("owned valid calibration history");`
- `crates/davout/src/reference_journal_namespace_tests.rs:21: .expect("positive legacy YAML load");`
- `crates/davout/src/reference_journal_namespace_tests.rs:40: let mut worker = Journal::spawn(journal.clone(), None).expect("real worker");`
- `crates/davout/src/reference_journal_namespace_tests.rs:46: .expect("actual captured input accepted");`
- `crates/davout/src/reference_journal_namespace_tests.rs:50: .expect("actual storage outcome");`
- `crates/davout/src/reference_journal_namespace_tests.rs:72: fs::create_dir(&actual).expect("owned parent");`
- `crates/davout/src/reference_journal_namespace_tests.rs:74: std::os::unix::fs::symlink(&actual, &alias).expect("owned parent alias");`
- `crates/davout/src/reference_codec_tests.rs:17: encode(&f64::from_bits(bits)).expect("f64 exact encoding"),`
- `crates/davout/src/reference_codec_tests.rs:22: .expect("f64 exact decode")`
- `crates/davout/src/reference_codec_tests.rs:30: encode(&f32::from_bits(bits)).expect("f32 exact encoding"),`
- `crates/davout/src/reference_codec_tests.rs:35: .expect("f32 exact decode")`
- `crates/davout/src/reference_codec_tests.rs:41: encode(&u64::MAX).expect("unsigned job width"),`
- `crates/davout/src/reference_codec_tests.rs:45: decode::<u64>(&encode(&u64::MAX).expect("u64")).expect("full u64"),`
- `crates/davout/src/reference_codec_tests.rs:49: decode::<i8>(&encode(&-1i8).expect("signed direction")).expect("i8"),`
- `crates/davout/src/reference_codec_tests.rs:52: let narrow = encode(&1u8).expect("narrow tag");`
- `crates/davout/src/reference_codec_tests.rs:74: assert_eq!(encode(&a).expect("canonical map"), expected);`
- `crates/davout/src/reference_codec_tests.rs:75: assert_eq!(encode(&b).expect("different input iteration"), expected);`
- `crates/davout/src/reference_journal_hardlink_tests.rs:20: let mut database = Database::open(&journal, true, true).expect("real existing database");`
- `crates/davout/src/reference_journal_hardlink_tests.rs:24: .expect("real retained body");`
- `crates/davout/src/reference_journal_hardlink_tests.rs:31: .expect("existing durable history");`
- `crates/davout/src/reference_journal_hardlink_tests.rs:33: let database_before = fs::read(&journal).expect("closed database bytes");`
- `crates/davout/src/reference_journal_hardlink_tests.rs:37: fs::write(&history, bytes).expect("owned valid YAML");`
- `crates/davout/src/reference_journal_hardlink_tests.rs:38: fs::hard_link(&history, &rollback).expect("actual shared file identity");`
- `crates/davout/src/reference_journal_hardlink_tests.rs:40: .expect("positive legacy history load");`
- `crates/davout/src/reference_journal_hardlink_tests.rs:58: let mut worker = Journal::spawn(journal.clone(), None).expect("actual worker");`
- `crates/davout/src/reference_journal_hardlink_tests.rs:64: .expect("actual captured input");`
- `crates/davout/src/reference_journal_hardlink_tests.rs:69: .expect("actual outcome")`
- `crates/davout/src/reference_journal_hardlink_tests.rs:81: fs::read(&journal).expect("preserved database") == database_before,`
- `crates/davout/src/reference_journal_hardlink_tests.rs:84: assert!(fs::read(&rollback).expect("preserved link") == bytes);`
- `crates/davout/src/reference_journal_hardlink_tests.rs:94: fs::write(&history, b"joints: []\n").expect("owned valid history");`
- `crates/davout/src/reference_journal_hardlink_tests.rs:95: fs::hard_link(&history, &link).expect("actual hard link");`
- `crates/davout/src/reference_journal_hardlink_tests.rs:115: fs::read(&history).expect("preserved history"),`
- `crates/davout/src/reference_journal_filetype_tests.rs:30: .expect("owned regular history");`
- `crates/davout/src/reference_journal_filetype_tests.rs:36: .expect("local named pipe")`
- `crates/davout/src/reference_journal_filetype_tests.rs:39: Command::new(std::env::current_exe().expect("actual test executable"))`
- `crates/davout/src/reference_journal_filetype_tests.rs:50: .expect("bounded child"),`
- `crates/davout/src/reference_journal_filetype_tests.rs:57: .expect("owned child")`
- `crates/davout/src/reference_journal_filetype_tests.rs:59: .expect("child status")`
- `crates/davout/src/reference_journal_filetype_tests.rs:68: .expect("owned child")`
- `crates/davout/src/reference_journal_filetype_tests.rs:70: .expect("terminate blocked fixture child");`
- `crates/davout/src/reference_journal_filetype_tests.rs:78: .expect("owned child")`
- `crates/davout/src/reference_journal_filetype_tests.rs:80: .expect("reaped child output");`
- `crates/davout/src/reference_journal_filetype_tests.rs:91: .expect("preserved unsupported resource")`
- `crates/davout/src/reference_journal_filetype_tests.rs:102: let suffix = std::env::var("MARENGO_JOURNAL_FIFO_SLOT").expect("fixed parent slot");`
- `crates/davout/src/reference_journal_filetype_tests.rs:117: let mut owner = constructed.expect("unsupported journal keeps lazy worker refusal");`
- `crates/davout/src/reference_journal_filetype_tests.rs:126: let mut worker = Journal::spawn(journal, None).expect("actual lazy worker");`
- `crates/davout/src/reference_journal_filetype_tests.rs:132: .expect("actual captured input");`
- `crates/davout/src/reference_journal_filetype_tests.rs:136: .expect("actual storage outcome");`
- `crates/davout/src/reference_journal_filetype_tests.rs:143: fs::read(root.join("history.yaml")).expect("preserved history"),`

## marengo-candump

### allow (1)
- `crates/marengo-candump/src/lib.rs:433: #[allow(clippy::unwrap_used)]`

### cfg_attr_allow (0)

(none)

### todo_fixme (0)

(none)

### todo_macro (0)

(none)

### ignore (0)

(none)

### let_underscore (1)
- `crates/marengo-candump/src/scan.rs:442: let _ = (can_id, interface);`

### unwrap (0)

(none)

### expect (0)

(none)

## marengo-config

### allow (1)
- `crates/marengo-config/src/urdf_merge.rs:72: #[allow(dead_code)]`

### cfg_attr_allow (0)

(none)

### todo_fixme (0)

(none)

### todo_macro (0)

(none)

### ignore (0)

(none)

### let_underscore (3)
- `crates/marengo-config/src/urdf_merge.rs:389: let _ = std::fs::remove_file(&tmp);`
- `crates/marengo-config/src/urdf_expand.rs:88: let _ = fs::remove_file(&temporary);`
- `crates/marengo-config/src/profile_txn.rs:314: let _ = fs::remove_file(path);`

### unwrap (0)

(none)

### expect (0)

(none)

## marengo-deploy

### allow (0)

(none)

### cfg_attr_allow (0)

(none)

### todo_fixme (0)

(none)

### todo_macro (0)

(none)

### ignore (0)

(none)

### let_underscore (5)
- `crates/marengo-deploy/src/job.rs:217: let _ = write_job_file(&job_path, &job);`
- `crates/marengo-deploy/src/status.rs:119: let _ = crate::job::write_job_file(&crate::paths::resolve_job_file_path(), &job);`
- `crates/marengo-deploy/src/upstream.rs:70: let _ = load_reconciled_job();`
- `crates/marengo-deploy/src/upstream.rs:192: let _ = std::fs::create_dir_all(parent);`
- `crates/marengo-deploy/src/upstream.rs:195: let _ = std::fs::write(path, body.to_string());`

### unwrap (0)

(none)

### expect (0)

(none)

## marengo-homing

### allow (2)
- `crates/marengo-homing/src/registry.rs:162: #[allow(clippy::too_many_arguments)]`
- `crates/marengo-homing/src/verify.rs:43: #[allow(clippy::too_many_arguments)]`

### cfg_attr_allow (0)

(none)

### todo_fixme (0)

(none)

### todo_macro (0)

(none)

### ignore (0)

(none)

### let_underscore (2)
- `crates/marengo-homing/src/registry.rs:120: let _ = message;`
- `crates/marengo-homing/src/sensor.rs:68: let _ = (gpio, active_high);`

### unwrap (0)

(none)

### expect (0)

(none)

## marengo-host-metrics

### allow (6)
- `crates/marengo-host-metrics/src/diagnostics.rs:37: #[allow(clippy::expect_used)]`
- `crates/marengo-host-metrics/src/diagnostics.rs:76: #[allow(clippy::expect_used)]`
- `crates/marengo-host-metrics/src/diagnostics.rs:264: #[allow(clippy::expect_used)]`
- `crates/marengo-host-metrics/src/diagnostics.rs:299: #[allow(clippy::expect_used)]`
- `crates/marengo-host-metrics/src/cpu.rs:84: #[allow(clippy::expect_used)]`
- `crates/marengo-host-metrics/src/cpu.rs:121: #[allow(clippy::expect_used)]`

### cfg_attr_allow (0)

(none)

### todo_fixme (0)

(none)

### todo_macro (0)

(none)

### ignore (0)

(none)

### let_underscore (1)
- `crates/marengo-host-metrics/src/lib.rs:58: let _ = (role, semver, prev, chappe);`

### unwrap (0)

(none)

### expect (0)

(none)

## marengo-imu

### allow (1)
- `crates/marengo-imu/src/shtp.rs:5: #[allow(dead_code)]`

### cfg_attr_allow (0)

(none)

### todo_fixme (0)

(none)

### todo_macro (0)

(none)

### ignore (0)

(none)

### let_underscore (1)
- `crates/marengo-imu/src/driver.rs:119: let _ = self.try_read_packet();`

### unwrap (0)

(none)

### expect (0)

(none)

## marengo-store

### allow (0)

(none)

### cfg_attr_allow (0)

(none)

### todo_fixme (0)

(none)

### todo_macro (0)

(none)

### ignore (0)

(none)

### let_underscore (3)
- `crates/marengo-store/src/journal.rs:32: let _ = (store, units);`
- `crates/marengo-store/src/store.rs:305: let _ = fs::remove_file(path);`
- `crates/marengo-store/src/store.rs:313: let _ = conn.execute_batch("PRAGMA wal_checkpoint(PASSIVE);");`

### unwrap (0)

(none)

### expect (0)

(none)

## robstride

### allow (0)

(none)

### cfg_attr_allow (0)

(none)

### todo_fixme (0)

(none)

### todo_macro (0)

(none)

### ignore (1)
- `crates/robstride/src/lib.rs:480: #[ignore = "requires vcan0/vcan1 from scripts/vcan-up.sh or just vcan"]`

### let_underscore (5)
- `crates/robstride/src/bus.rs:1068: let _ = interface;`
- `crates/robstride/src/bus.rs:1083: let _ = motors;`
- `crates/robstride/src/bus.rs:1094: let _ = address;`
- `crates/robstride/src/bus.rs:1106: let _ = frame;`
- `crates/robstride/src/bus.rs:1118: let _ = (address, frame);`

### unwrap (0)

(none)

### expect (0)

(none)
