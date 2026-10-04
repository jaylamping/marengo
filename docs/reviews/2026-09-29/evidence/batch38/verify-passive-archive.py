from pathlib import Path
import collections, hashlib, json, re, tarfile

work = Path('J:/code/marengo-migration-backup-20260929/batch38-pi-receive-diagnostics')
archive = work / 'batch38-passive-owner-20261002.tar.gz'
receipt = json.loads((work / 'batch38-passive-owner-20261002-archive.json').read_text())
assert archive.stat().st_size == receipt['bytes']
assert hashlib.sha256(archive.read_bytes()).hexdigest() == receipt['SHA256']
destination = work / 'passive-owner-observation'
assert not destination.exists()
with tarfile.open(archive, 'r:gz') as tar:
    expected = {entry['name']: entry for entry in receipt['members']}
    assert len(expected) == 8
    assert sorted(m.name for m in tar.getmembers()) == sorted(expected)
    contents = {}
    for member in tar.getmembers():
        assert member.isfile() and Path(member.name).name == member.name
        raw = tar.extractfile(member).read()
        binding = expected[member.name]
        assert len(raw) == binding['bytes']
        assert hashlib.sha256(raw).hexdigest() == binding['SHA256']
        contents[member.name] = raw
destination.mkdir()
for name, raw in contents.items():
    (destination / name).write_bytes(raw)
owner = json.loads(contents['passive-owner-receipt.json'])
raw = contents['passive-can.log']
assert hashlib.sha256(raw).hexdigest() == owner['capture_SHA256']
assert not contents['passive-can.stderr']
frames = []
kinds = collections.Counter()
errors = []
for line in raw.decode().splitlines():
    match = re.fullmatch(r'\((\d+\.\d+)\)\s+(can[01])\s+([0-9A-Fa-f]+)#([0-9A-Fa-f]*)', line)
    assert match, line
    stamp, can_id, data = float(match[1]), int(match[3], 16), match[4]
    assert len(data) <= 16 and len(data) % 2 == 0
    frames.append(stamp)
    if can_id & 0x20000000:
        errors.append(line)
    else:
        kind = (can_id >> 24) & 31
        kinds[kind] += 1
        assert kind not in (3, 6), line
        if kind == 1:
            assert data == '7FFF7FFF00000000', line
assert frames[-1] - frames[0] >= 295
assert len(owner['samples']) == 297
assert all(s['disabled'] and not s['software_latch'] and not s['messages'] for s in owner['samples'])
assert len(owner['changes']) == 1 and owner['changes'][0]['messages'] == []
def counter(name):
    match = re.search(r'RX:\s+bytes\s+packets\s+errors\s+dropped\s+missed\s+mcast\s+\n\s*\d+\s+\d+\s+(\d+)', contents[name].decode())
    assert match
    return int(match[1])
summary = {
    'started_UTC': owner['started_UTC'], 'ended_UTC': owner['ended_UTC'],
    'installed_revision': owner['installed_revision'], 'owner_PID_unchanged': owner['owner_PID_unchanged'],
    'archive_SHA256': receipt['SHA256'], 'archive_members_verified': 8,
    'capture_SHA256': owner['capture_SHA256'], 'capture_seconds': frames[-1] - frames[0],
    'frames': len(frames), 'error_frames': errors, 'transport_frame_kinds': dict(sorted(kinds.items())),
    'Enable_and_SetZero_frames': 0, 'nonneutral_MIT_frames': 0,
    'fresh_Disabled_samples': len(owner['samples']), 'software_latch_samples': 0,
    'initial_and_only_message_set': [], 'CAN0_rx_errors': [counter('before-can0.txt'), counter('after-can0.txt')],
    'physical_acceptance': False,
    'scope': 'One bounded passive observation of the existing owner; no restart, build, load injection or motor command. No new error to correlate with the owner diagnostic; intermittent CAN cause, recovery, reference and motion remain unqualified.'
}
(work / 'passive-owner-analysis.json').write_text(json.dumps(summary, indent=2) + '\n', encoding='utf-8')
print(json.dumps(summary, indent=2))
