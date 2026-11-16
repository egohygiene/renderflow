#!/usr/bin/env python3
"""Observe requested ALAC precision through a captured canonical Renderflow CLI.

The retained decoder comparison uses integer source samples for quantization;
test-only producer faults never change the independently pinned verifiers.
"""
from __future__ import annotations
import argparse
from datetime import datetime, timezone
from fractions import Fraction
import hashlib
import importlib.util
import json
import os
from pathlib import Path
import shutil
import sys
import wave
sys.dont_write_bytecode = True

NATIVE = 'validator.audio.pinned_pcm_delivery'
CODECS = {'flac': 'flac', 'm4a_alac': 'alac', 'm4a': 'aac', 'mp3': 'mp3'}


class Observation:
    def __init__(self, binary, tools, work, helper, observer):
        spec = importlib.util.spec_from_file_location('authored_audio_helper', helper)
        self.base = importlib.util.module_from_spec(spec)
        spec.loader.exec_module(self.base)
        self.require = self.base.require
        self.hashed = self.base.hashed
        self.put = self.base.put
        self.binary = binary.resolve(strict=True)
        self.tools = tools.resolve(strict=True)
        self.helper = helper.resolve(strict=True)
        self.observers = [observer.resolve(strict=True)]
        if Path(__file__).resolve() not in self.observers:
            self.observers.append(Path(__file__).resolve())
        self.source_pins = {str(p): self.pin(p) for p in [self.binary, self.helper, *self.observers]}
        self.tool_pins = {name: self.pin(self.tools / name) for name in ('ffmpeg', 'ffprobe')}
        self.require(self.tool_pins['ffmpeg']['sha256'] == self.base.FFMPEG_SHA
                     and self.tool_pins['ffprobe']['sha256'] == self.base.FFPROBE_SHA, 'wrong pinned audio verifiers')
        self.work = work.absolute()
        self.require(self.work.parent.resolve() == self.work.parent and self.work.parent.is_dir(),
                     'canonical existing observation parent required')
        self.work.mkdir(mode=0o700)
        self.inputs, self.outputs, self.logs = [self.work / p for p in ('inputs', 'outputs', 'observed')]
        for p in (self.inputs, self.outputs, self.logs):
            p.mkdir()
        self.env = dict(os.environ, PATH=str(self.tools) + os.pathsep + os.environ['PATH'],
                        LC_ALL='C', NO_COLOR='1', TERM='dumb')
        self.events, self.cases, self.sources, self.feature_cache = [], [], {}, {}

    def pin(self, path):
        return {'bytes': path.stat().st_size, 'sha256': self.hashed(path)}

    def normalized(self, value):
        return str(value).replace(str(self.binary), '{renderflow}').replace(str(self.work), '{work}').replace(str(self.tools), '{audio-tools}')

    def run(self, name, argv, code=0, env=None):
        self.require(name not in {event['name'] for event in self.events}, 'duplicate observation label')
        result = self.base.capture(list(map(str, argv)), self.inputs, env or self.env, timeout=60)
        (self.logs / (name + '.stdout')).write_bytes(result.stdout)
        (self.logs / (name + '.stderr')).write_bytes(result.stderr)
        event = {'name': name, 'argv': [self.normalized(v) for v in argv], 'exit_code': result.returncode,
                 'stdout_sha256': hashlib.sha256(result.stdout).hexdigest(),
                 'stderr_sha256': hashlib.sha256(result.stderr).hexdigest()}
        self.events.append(event)
        self.require(result.returncode == code, 'unexpected process exit ' + name)
        return result

    def source(self, name, depth=24, rate=44100, channels=2):
        raw = bytearray()
        frames = rate * 3
        scale = 1 << (depth - 5)
        for frame in range(frames):
            for channel in range(channels):
                period, phase = ((200, 0), (307, 61))[channel]
                n = (frame + phase) % period
                value = (2 * min(n, period - n) - period // 2) * scale // period
                raw.extend(value.to_bytes(depth // 8, 'little', signed=True))
        path = self.inputs / (name + '.wav')
        with wave.open(str(path), 'wb') as stream:
            stream.setnchannels(channels)
            stream.setsampwidth(depth // 8)
            stream.setframerate(rate)
            stream.writeframes(raw)
        with wave.open(str(path), 'rb') as stream:
            self.require((stream.getsampwidth(), stream.getframerate(), stream.getnchannels(), stream.getnframes())
                         == (depth // 8, rate, channels, frames), 'authored WAV layout differs')
            self.require(stream.readframes(frames) == raw, 'authored WAV payload differs')
        value = {'path': path, 'depth': depth, 'rate': rate, 'channels': channels, 'frames': frames,
                 'pcm': bytes(raw), 'file_pin': self.pin(path)}
        self.sources[name] = value
        return value

    @staticmethod
    def targets(*items):
        return [{'id': fmt, 'role': 'delivery/' + fmt, 'format': fmt, 'preset': preset} for fmt, preset in items]

    def config(self, name, source, targets, native=True, execution=None):
        validation = {}
        if native:
            validation['audio'] = {'profile': 'pcm-delivery-ffmpeg-8.0.1-v1', **{
                name: {'path': str(self.tools / name), 'sha256': self.tool_pins[name]['sha256']}
                for name in ('ffmpeg', 'ffprobe')}}
        value = {'schema': 'renderflow/v2', 'sources': [{'id': 'authored-audio', 'path': str(source['path']), 'format': 'wav'}],
                 'targets': {'exact': targets}, 'execution': {'max_parallel': 1, 'validation': validation},
                 'output': {'bundle_root': str(self.outputs / name)}}
        value['execution'].update(execution or {})
        path = self.inputs / (name + '.json')
        self.put(path, value)
        return path

    def build(self, name, config, state='complete', env=None):
        root = Path(json.loads(config.read_bytes())['output']['bundle_root'])
        result = self.run(name, [self.binary, 'build', '--config', config], 0 if state == 'complete' else 1, env)
        manifest = root / 'renderflow-run.json'
        value = json.loads(manifest.read_bytes())
        self.require(value['state'] == state, 'wrong structured build state ' + name)
        if state != 'complete':
            tail = f"Error: renderflow execution finished with state {state.title()}; inspect '{manifest}' for structured evidence\n".encode()
            self.require(result.stderr.endswith(tail), 'wrong owning build refusal ' + name)
        snapshot = self.logs / (name + '.manifest.json')
        shutil.copyfile(manifest, snapshot)
        rows = [r for r in value['artifact_manifest']['artifacts'] if r['lifecycle'] == 'terminal']
        case = {'name': name, 'state': state, 'manifest': self.normalized(manifest), 'manifest_pin': self.pin(manifest),
                'retained_raw_manifest': self.normalized(snapshot), 'published': len(value['artifact_manifest']['outputs']),
                'targets': [{'format': r['format'], 'artifact_id': r['artifact_id'], 'locator': self.normalized(r['locator']),
                             'validation': r['validation'], 'fidelity': r['fidelity'],
                             'published': r['metadata'].get('renderflow.target.published'),
                             'validators': r.get('validation_evidence', []),
                             'native': r['metadata'].get('renderflow.audio.native_validation')} for r in rows]}
        self.cases.append(case)
        return root, value, rows, case

    def native(self, row, state='valid', code=None):
        records = [r for r in row['validation_evidence'] if r['validator_id'] == NATIVE]
        self.require(len(records) == 1 and records[0]['state'] == state, 'missing/duplicate/wrong native validator state')
        if code:
            self.require([d['code'] for d in records[0]['diagnostics']] == [code], 'wrong native refusal code')
        return row['metadata'].get('renderflow.audio.native_validation')

    def features(self, admission):
        self.require(admission['admitted'] and len(admission['queries']) == 6
                     and all(r['matched'] for r in admission['required']), 'feature admission is not closed/complete')
        for query in admission['queries']:
            self.require(query['capture_complete'] and query['recognized'], 'unrecognized or truncated feature evidence')
            name = 'ffmpeg' if query['tool_id'] == 'tool.ffmpeg' else 'ffprobe'
            key = (name, tuple(query['argv']))
            if key not in self.feature_cache:
                result = self.run('features-' + str(len(self.feature_cache)), [self.tools / name, *query['argv']])
                self.feature_cache[key] = (hashlib.sha256(result.stdout).hexdigest(), hashlib.sha256(result.stderr).hexdigest())
            self.require(self.feature_cache[key] == (query['stdout_sha256'], query['stderr_sha256']), 'feature bytes differ')

    def inspect(self, name, source, root, manifest, row, mode):
        native = self.native(row)
        self.require(row['validation'] == 'valid' and row['metadata']['renderflow.target.published'] is True,
                     'native positive was not accepted/published')
        source_row = next(r for r in manifest['artifact_manifest']['artifacts'] if r['lifecycle'] == 'source')
        branch = next(b for b in manifest['artifact_forest']['branches'] if b['target_id'] == row['metadata']['renderflow.target.identity'])
        step = next(s for s in manifest['steps'] if s['step_id'] == native['producing_step_id'])
        self.require(native['target_identity'] == branch['target_id'] and native['requested_audio_profile'] == branch['preset']
                     and native['artifact_id'] == row['artifact_id'] and native['artifact_sha256'] == row['digest']['value']
                     and native['source_artifact_id'] == source_row['artifact_id']
                     and native['source_sha256'] == source_row['digest']['value'] == self.hashed(source['path'])
                     and native['producing_step_configuration_sha256'] == step['configuration_digest']['value']
                     and row['sources'] == [source_row['artifact_id']], 'source/target/profile/step identity detached')
        self.require(native['source_pcm'] == {'precision_bits': source['depth'], 'sample_rate_hz': source['rate'],
                                              'channels': source['channels'], 'frames': source['frames'],
                                              'sample_sha256': hashlib.sha256(source['pcm']).hexdigest()}, 'native source PCM detached')
        self.require(native['verifier_ffmpeg']['sha256'] == self.base.FFMPEG_SHA
                     and native['verifier_ffprobe']['sha256'] == self.base.FFPROBE_SHA
                     and all(native[k] is False for k in ['encoded_bitrate_or_compression_attested',
                             'encoder_executable_hash_attested', 'native_library_or_os_sandbox_attested',
                             'mastering_playback_or_content_approval']), 'native authority changed')
        self.features(native['feature_admission'])
        self.require(row['locator'].startswith('bundle:'), 'accepted artifact is not published bundle member')
        artifact = root / row['locator'][7:]
        self.require(self.hashed(artifact) == row['digest']['value'], 'published payload differs')
        label = name + '-' + row['format']
        probe = json.loads(self.run(label + '-probe', [self.tools / 'ffprobe', '-v', 'error', '-show_streams',
                                                     '-show_packets', '-of', 'json', artifact]).stdout)
        self.require(len(probe['streams']) == 1 and probe['packets'], 'wrong native media inventory')
        stream, packets, clock = probe['streams'][0], probe['packets'], native['observed_clock']
        rate, channels = int(stream['sample_rate']), stream['channels']
        self.require(stream['codec_name'] == CODECS[row['format']] and rate == clock['rate_hz']
                     and channels == clock['channels'], 'independent codec/rate/channels differ')
        if row['format'] in ('m4a_alac', 'flac'):
            self.require(int(stream['bits_per_raw_sample']) == native['decoded_precision_bits'], 'independent precision differs')
        tick = Fraction(stream['time_base']) * rate
        self.require(Fraction(stream['duration_ts']) * tick == clock['stream_duration_samples']
                     and Fraction(packets[0]['pts']) * tick == clock['packet_start_sample']
                     and (Fraction(packets[-1]['pts']) + int(packets[-1]['duration'])) * tick == clock['packet_end_sample'],
                     'independent stream/packet extent differs')
        skip = sum(int(d.get('skip_samples', 0)) for p in packets for d in p.get('side_data_list', []) if d['side_data_type'] == 'Skip Samples')
        discard = sum(int(d.get('discard_padding', 0)) for p in packets for d in p.get('side_data_list', []) if d['side_data_type'] == 'Skip Samples')
        self.require(skip == clock['skip_samples'] and discard == clock['discard_padding']
                     and clock['packet_end_sample'] - clock['packet_start_sample'] - skip - discard == clock['presentation_samples'],
                     'independent presentation reconciliation differs')
        for index, packet in enumerate(packets):
            span = Fraction(packet['duration']) * tick
            self.require(packet['codec_type'] == 'audio' and span > 0, 'wrong packet media/extent')
            if index:
                previous = packets[index - 1]
                self.require(Fraction(packet['pts']) == Fraction(previous['pts']) + int(previous['duration']), 'packet discontinuity')
            if row['format'] == 'mp3':
                self.require(span == 1152, 'wrong MPEG-1 frame extent')
            if row['format'] == 'm4a':
                self.require(span <= 1024 and (index + 1 == len(packets) or span == 1024), 'wrong AAC frame extent')
        expected_extent = Fraction(source['frames'] * rate, source['rate'])
        floor = expected_extent.numerator // expected_extent.denominator
        ceiling = (expected_extent.numerator + expected_extent.denominator - 1) // expected_extent.denominator
        self.require(clock['presentation_samples'] in (floor, ceiling), 'presentation differs from source/resample extent')
        bits = native['decoded_precision_bits']
        raw = self.logs / (label + '.pcm')
        self.run(label + '-decode', [self.tools / 'ffmpeg', '-v', 'error', '-i', artifact, '-map', '0:a:0',
                                    '-c:a', f'pcm_s{bits}le', '-f', f's{bits}le', raw])
        self.require(raw.stat().st_size % (bits // 8 * channels) == 0
                     and self.hashed(raw) == native['decoded_pcm_sha256']
                     and raw.stat().st_size // (bits // 8 * channels) == native['decoded_frames'], 'independent decoded PCM differs')
        expected_fidelity = 'lossless' if mode == 'exact' and row['format'] in ('flac', 'm4a_alac') else 'lossy'
        self.require(row['fidelity'] == step['fidelity'] == native['fidelity'] == expected_fidelity, 'incorrect observed fidelity')
        if mode == 'exact':
            self.require((bits, rate, channels, native['decoded_frames'])
                         == (source['depth'], source['rate'], source['channels'], source['frames'])
                         and raw.read_bytes() == source['pcm'] and native['decoded_samples_equal_original']
                         and not native['profile_changes_pcm_layout'], 'exact source-preserving PCM claim failed')
        if mode == 'quantize16':
            self.require((source['depth'], bits, rate, channels) == (24, 16, source['rate'], source['channels']), 'wrong quantization layout')
            expected = b''.join((int.from_bytes(source['pcm'][i:i + 3], 'little', signed=True) >> 8).to_bytes(2, 'little', signed=True)
                                for i in range(0, len(source['pcm']), 3))
            discarded = sum(source['pcm'][i] != 0 for i in range(0, len(source['pcm']), 3))
            self.require(discarded > 0 and raw.read_bytes() == expected and native['profile_changes_pcm_layout']
                         and not native['decoded_samples_equal_original'], 'ALAC quantization did not match integer source projection')
            expected_path = self.logs / (label + '.integer-expected.pcm')
            expected_path.write_bytes(expected)
            return {'independent_quantization': 'signed source24 >> 8, interleaved little-endian16',
                    'discarded_nonzero_low_bytes': discarded, 'expected_pcm': self.pin(expected_path)}
        if mode == 'converted':
            self.require(native['profile_changes_pcm_layout'] and not native['decoded_samples_equal_original'], 'declared conversion became preserving')
        return {}

    def positive(self, name, source, targets, modes):
        config = self.config(name, source, targets)
        root, manifest, rows, case = self.build(name, config)
        self.require(len(rows) == len(targets) and len({r['artifact_id'] for r in rows}) == len(rows)
                     and len({r['locator'] for r in rows}) == len(rows), 'target identities collided')
        self.require({str(root / r['locator'][7:]) for r in rows} == set(manifest['artifact_manifest']['outputs']), 'published output inventory differs')
        for row, summary in zip(rows, case['targets']):
            summary['independent_observation'] = self.inspect(name, source, root, manifest, row, modes[row['format']])
        return config, root, manifest, rows

    def producer_fault(self, name, change):
        directory = self.inputs / (name + '-producer')
        directory.mkdir()
        marker = self.logs / (name + '-producer-events.jsonl')
        script = directory / 'ffmpeg'
        code = "#!/usr/bin/python3\nimport json,os,sys\n"
        code += 'args=sys.argv[1:]\nchange=' + repr(change) + '\n'
        code += "if '-acodec' in args and args[args.index('-acodec')+1]=='alac':\n"
        code += " before=list(args)\n"
        code += " if change.get('omit_precision'):\n  i=args.index('-sample_fmt');del args[i:i+2]\n"
        code += " if 'sample_format' in change: args[args.index('-sample_fmt')+1]=change['sample_format']\n"
        code += " if 'rate' in change: args[args.index('-ar')+1]=str(change['rate'])\n"
        code += " if 'filter' in change: args[-1:-1]=['-af',change['filter']]\n"
        code += ' with open(' + repr(str(marker)) + ",'a') as stream:stream.write(json.dumps({'before':before,'after':args})+'\\n')\n"
        code += 'os.execv(' + repr(str(self.tools / 'ffmpeg')) + ',[' + repr(str(self.tools / 'ffmpeg')) + ',*args])\n'
        script.write_text(code)
        script.chmod(0o700)
        env = dict(self.env, PATH=str(directory) + os.pathsep + self.env['PATH'])
        self.require(Path(shutil.which('ffmpeg', path=env['PATH'])).resolve() == script, 'controlled producer not selected')
        return env, marker, script

    def refused(self, name, source, targets, code, change):
        config = self.config(name, source, targets)
        # A distinct CAS parent makes this a real producer run. Reusing a prior
        # correct cached payload cannot exercise an intentionally faulty encoder.
        parent = self.outputs / name
        parent.mkdir()
        specification = json.loads(config.read_bytes())
        specification['output']['bundle_root'] = str(parent / 'bundle')
        self.put(config, specification)
        env, marker, script = self.producer_fault(name, change)
        script_before = self.pin(script)
        root, manifest, rows, case = self.build(name, config, 'failed', env)
        self.require(len(rows) == 1 and not manifest['artifact_manifest']['outputs'], 'refused producer output was accepted')
        self.native(rows[0], 'invalid', code)
        self.require(rows[0]['metadata']['renderflow.target.published'] is False
                     and marker.is_file() and self.pin(script) == script_before, 'wrong producer-control refusal')
        events = [json.loads(line) for line in marker.read_text().splitlines()]
        self.require(len(events) == 1 and events[0]['before'] != events[0]['after'], 'controlled producer fault did not execute once')
        row = rows[0]
        self.require(row['locator'].startswith('artifact-store:'), 'refused CAS output missing')
        artifact = root.parent / '.renderflow/artifacts' / row['locator'][15:]
        self.require(self.hashed(artifact) == row['digest']['value'], 'refused CAS output changed')
        probe = json.loads(self.run(name + '-fault-probe', [self.tools / 'ffprobe', '-v', 'error', '-show_streams',
                                                          '-show_packets', '-of', 'json', artifact]).stdout)
        case['test_only_producer_fault'] = {'change': change, 'script': self.pin(script), 'marker': self.pin(marker),
                                             'observed_stream': probe['streams'][0], 'verifiers_unchanged': True,
                                             'producer_hash_attested_by_product': False}
        return probe, rows[0]

    def finish(self, schema, scope):
        for source in self.sources.values():
            self.require(self.pin(source['path']) == source['file_pin'], 'original source changed')
        self.require({name: self.pin(self.tools / name) for name in self.tool_pins} == self.tool_pins, 'pinned verifier changed')
        for path, pin in self.source_pins.items():
            self.require(self.pin(Path(path)) == pin, 'observer/helper/candidate changed')
        report = {'schema_version': schema, 'observed_at_utc': datetime.now(timezone.utc).isoformat(),
                  'binary_sha256': self.source_pins[str(self.binary)]['sha256'],
                  'observer_pins': {Path(p).name: pin for p, pin in self.source_pins.items() if Path(p) != self.binary},
                  'verifier_subjects': self.tool_pins, 'events': self.events, 'cases': self.cases,
                  'source_inventory': {name: {'depth': v['depth'], 'rate': v['rate'], 'channels': v['channels'],
                                              'frames': v['frames'], 'file': v['file_pin'],
                                              'pcm_sha256': hashlib.sha256(v['pcm']).hexdigest()} for name, v in self.sources.items()},
                  'input_inventory': self.base.inventory(self.inputs), 'output_inventory': self.base.inventory(self.outputs),
                  'observation_inventory': self.base.inventory(self.logs), 'scope': scope,
                  'encoder_executable_hash_attested': False, 'native_platform_matrix_qualified': False,
                  'mastering_loudness_or_creative_approval': False, 'remote_publication_performed': False}
        self.put(self.work / 'report.json', report)
        print(json.dumps({'state': 'passed', 'cases': len(self.cases), 'commands': len(self.events),
                          'binary_sha256': report['binary_sha256']}, sort_keys=True))
        return report


def main(binary, tools, work, helper):
    o = Observation(binary, tools, work, helper, Path(__file__))
    source24, source16 = o.source('source24', 24), o.source('source16', 16)
    _, _, _, rows = o.positive('quantized-cd', source24, o.targets(('m4a_alac', 'alac_cd'), ('m4a', 'aac_256k')),
                              {'m4a_alac': 'quantize16', 'm4a': 'lossy'})
    o.require({Path(r['locator']).suffix for r in rows} == {'.m4a'}, 'same-container suffix changed')
    o.positive('matching-cd', source16, o.targets(('m4a_alac', 'alac_cd')), {'m4a_alac': 'exact'})
    o.positive('explicit-24bit-conversion', source16, o.targets(('m4a_alac', 'alac_broadcast')), {'m4a_alac': 'converted'})
    config = o.config('quantization-loss-gate', source24, o.targets(('m4a_alac', 'alac_cd')),
                      execution={'reject_loss_classes': ['lossy']})
    _, manifest, rows, _ = o.build('quantization-loss-gate', config, 'failed')
    native = o.native(rows[0])
    o.require(native['fidelity'] == rows[0]['fidelity'] == 'lossy' and not manifest['artifact_manifest']['outputs']
              and rows[0]['metadata']['renderflow.target.published'] is False, 'quantization bypassed existing loss gate')
    probe, _ = o.refused('omitted-cd-precision', source24, o.targets(('m4a_alac', 'alac_cd')),
                         'audio.native.profile.layout', {'omit_precision': True})
    o.require(probe['streams'][0]['codec_name'] == 'alac' and probe['streams'][0]['bits_per_raw_sample'] == '24',
              'controlled legacy encoder failure did not retain24-bit precision')
    config = o.config('legacy-validation', source24, o.targets(('m4a_alac', 'alac_cd')), native=False)
    _, _, rows, _ = o.build('legacy-validation', config)
    o.require(rows[0]['validation'] == 'valid_with_warnings' and rows[0]['fidelity'] == 'lossy'
              and all(v['validator_id'] != NATIVE for v in rows[0]['validation_evidence']), 'legacy generic validation changed')
    return o.finish('renderflow.alac-precision-cli-proof/v1',
                    'Requested ALAC precision and exact integer PCM comparisons only. Controlled faulty producer is test-only; verifier identities remain separately pinned.')


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    for name in ('binary', 'audio-tool-directory', 'work-directory'):
        parser.add_argument('--' + name, type=Path, required=True)
    parser.add_argument('--observer-helper', type=Path, default=Path(__file__).with_name('check_audio_delivery_candidate.py'))
    args = parser.parse_args()
    main(args.binary, args.audio_tool_directory, args.work_directory, args.observer_helper)
