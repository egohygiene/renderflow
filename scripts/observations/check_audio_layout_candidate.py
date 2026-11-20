#!/usr/bin/env python3
"""Observe new native audio layouts with the exact ALAC-precision candidate.

No new provider is installed. Controlled producer faults use independently pinned
real verifiers, and supported conversions retain their declared lossy scope.
"""
from __future__ import annotations
import argparse
from fractions import Fraction
import importlib.util
from pathlib import Path
import sys
sys.dont_write_bytecode = True


def main(binary, tools, work, helper, precision_helper):
    spec = importlib.util.spec_from_file_location('precision_observer', precision_helper)
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    o = module.Observation(binary, tools, work, helper, Path(__file__))
    mono = o.source('source-48000-mono24', 24, 48000, 1)
    stereo = o.source('source-96000-stereo24', 24, 96000, 2)
    selected = [('flac', 'flac_broadcast'), ('m4a_alac', 'alac_broadcast'),
                ('m4a', 'aac_256k'), ('mp3', 'hq_320k')]
    _, _, _, mono_rows = o.positive('native-48000-mono24', mono, o.targets(*selected),
                                    {'flac': 'exact', 'm4a_alac': 'exact', 'm4a': 'lossy', 'mp3': 'converted'})
    for row in mono_rows:
        native = o.native(row)
        o.require(native['observed_clock']['channels'] == 1, 'mono source was silently widened')
        o.require(native['observed_clock']['rate_hz'] == (44100 if row['format'] == 'mp3' else 48000), 'wrong selected mono rate')
    selected[:2] = [('flac', 'flac_hires'), ('m4a_alac', 'alac_hires')]
    _, _, _, stereo_rows = o.positive('native-96000-stereo24', stereo, o.targets(*selected),
                                      {'flac': 'exact', 'm4a_alac': 'exact', 'm4a': 'lossy', 'mp3': 'converted'})
    for row in stereo_rows:
        native = o.native(row)
        o.require(native['observed_clock']['channels'] == 2, 'stereo source layout changed')
        o.require(native['observed_clock']['rate_hz'] == (44100 if row['format'] == 'mp3' else 96000), 'wrong selected stereo rate')
    target = o.targets(('m4a_alac', 'alac_hires'))
    probe, _ = o.refused('wrong-96k-precision', stereo, target, 'audio.native.profile.layout', {'sample_format': 's16p'})
    o.require(probe['streams'][0]['bits_per_raw_sample'] == '16', 'wrong-precision producer fault was not observed')
    probe, _ = o.refused('wrong-96k-rate', stereo, target, 'audio.native.profile.layout', {'rate': 48000})
    o.require(int(probe['streams'][0]['sample_rate']) == 48000, 'wrong-rate producer fault was not observed')
    probe, _ = o.refused('trimmed-96k-clock', stereo, target, 'audio.native.presentation_mismatch',
                         {'filter': 'atrim=end_sample=' + str(stereo['frames'] - 1)})
    stream = probe['streams'][0]
    o.require(int(stream['sample_rate']) == stereo['rate'] and stream['channels'] == stereo['channels']
              and int(stream['bits_per_raw_sample']) == stereo['depth']
              and Fraction(stream['duration_ts']) * Fraction(stream['time_base']) * stereo['rate'] == stereo['frames'] - 1,
              'one-frame producer clock fault was not independently observed')
    probe, row = o.refused('changed-96k-pcm', stereo, target, 'audio.native.pcm_mismatch',
                           {'filter': 'aformat=sample_fmts=s16p'})
    stream = probe['streams'][0]
    o.require(int(stream['sample_rate']) == stereo['rate'] and stream['channels'] == stereo['channels']
              and int(stream['bits_per_raw_sample']) == stereo['depth']
              and Fraction(stream['duration_ts']) * Fraction(stream['time_base']) * stereo['rate'] == stereo['frames'],
              'PCM fault also changed the admitted layout/clock')
    cas = o.outputs / 'changed-96k-pcm/.renderflow/artifacts' / row['locator'][15:]
    raw = o.logs / 'changed-96k-pcm.independent.pcm'
    o.run('changed-96k-pcm-independent-decode', [o.tools / 'ffmpeg', '-v', 'error', '-i', cas,
                                               '-map', '0:a:0', '-c:a', 'pcm_s24le', '-f', 's24le', raw])
    expected = b''.join(((int.from_bytes(stereo['pcm'][i:i + 3], 'little', signed=True) >> 8) << 8).to_bytes(3, 'little', signed=True)
                        for i in range(0, len(stereo['pcm']), 3))
    o.require(raw.stat().st_size == len(stereo['pcm']) and raw.read_bytes() == expected and expected != stereo['pcm'],
              'independent PCM quantization fault was not observed')
    o.cases[-1]['independent_pcm_fault'] = 'Every source24 sample is reduced to16 then promoted to24; integer low8 bits become zero while precision/rate/channels/extent remain unchanged.'
    unsupported = o.source('source-96000-stereo32', 32, 96000, 2)
    config = o.config('unsupported-96k-integer32', unsupported, target)
    _, manifest, rows, _ = o.build('unsupported-96k-integer32', config, 'failed')
    o.require(not manifest['artifact_manifest']['outputs'] and len(rows) == 1, 'unsupported source published')
    o.native(rows[0], 'unavailable', 'audio.native.source_precision_unsupported')
    return o.finish('renderflow.native-audio-layout-cli-proof/v1',
                    'Authored48kHz mono24 and96kHz stereo24 direct WAV profiles only, with exact PCM and explicit selected rate conversion. Test-only faulty producers prove unchanged native layout/clock/PCM refusal; unsupported integer32 remains unavailable.')


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    for name in ('binary', 'audio-tool-directory', 'work-directory'):
        parser.add_argument('--' + name, type=Path, required=True)
    parser.add_argument('--observer-helper', type=Path, default=Path(__file__).with_name('check_audio_delivery_candidate.py'))
    parser.add_argument('--precision-observer', type=Path, default=Path(__file__).with_name('check_alac_precision_candidate.py'))
    args = parser.parse_args()
    main(args.binary, args.audio_tool_directory, args.work_directory, args.observer_helper, args.precision_observer)
