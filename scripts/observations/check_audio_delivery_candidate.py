#!/usr/bin/env python3
"""Actual authored PCM WAV delivery through existing canonical audio strategies.

Python standard library only. Pinned FFmpeg8.0.1/FFprobe8.0.1 are selected per
invocation; no mastering, loudness, physical playback or perceptual approval.
"""
from __future__ import annotations
import argparse
from datetime import datetime, timezone
from fractions import Fraction
import hashlib
import json
import os
from pathlib import Path
import selectors
import shutil
import signal
import stat
import subprocess
import time
import wave

FFMPEG_SHA='94dbc5c9d7602f044b0ec9089cea2085d0f2adc3a2b1612eeec84f8badc6949d'
FFPROBE_SHA='40d68a7e8fc61e8656be62b12179142124ebd61eea5ed8afefda6e2d68b97a1d'
RATE=44100
FRAMES=RATE*3

def require(value,message):
    if not value:raise ValueError(message)

def sha(data):return hashlib.sha256(data).hexdigest()

def hashed(path):
    digest=hashlib.sha256()
    with path.open('rb') as stream:
        for block in iter(lambda:stream.read(1024*1024),b''):digest.update(block)
    return digest.hexdigest()

def put(path,value):path.write_text(json.dumps(value,indent=2,sort_keys=True)+'\n',encoding='utf-8')

def inventory(root):
    result={}
    for path in sorted(root.rglob('*')):
        mode=path.lstat().st_mode
        if stat.S_ISDIR(mode):continue
        require(stat.S_ISREG(mode),'special file in accepted audio tree')
        result[str(path.relative_to(root))]={'bytes':path.stat().st_size,'sha256':hashed(path)}
    return result

def capture(argv,cwd,env,timeout=60,maximum=4*1024*1024):
    process=subprocess.Popen(argv,cwd=cwd,env=env,stdin=subprocess.DEVNULL,stdout=subprocess.PIPE,stderr=subprocess.PIPE,start_new_session=True)
    selector=selectors.DefaultSelector();streams={'stdout':bytearray(),'stderr':bytearray()};completed=False
    try:
        for name in streams:
            stream=getattr(process,name);os.set_blocking(stream.fileno(),False);selector.register(stream,selectors.EVENT_READ,name)
        deadline=time.monotonic()+timeout;total=0
        while selector.get_map():
            remaining=deadline-time.monotonic();require(remaining>0,'audio observation timed out')
            for key,_ in selector.select(min(remaining,.1)):
                data=os.read(key.fileobj.fileno(),65536)
                if not data:selector.unregister(key.fileobj);continue
                total+=len(data);require(total<=maximum,'audio capture limit exceeded');streams[key.data].extend(data)
        remaining=deadline-time.monotonic();require(remaining>0,'audio process exit timed out')
        code=process.wait(timeout=remaining);completed=True
        return subprocess.CompletedProcess(argv,code,bytes(streams['stdout']),bytes(streams['stderr']))
    finally:
        try:os.killpg(process.pid,signal.SIGKILL)
        except ProcessLookupError:pass
        if process.poll() is None:process.wait()
        selector.close()
        for name in streams:getattr(process,name).close()

def authored(path,depth):
    """Integer-only stereo triangle samples, channel-distinct and reproducible."""
    raw=bytearray(); scale=1<<(depth-5)
    for frame in range(FRAMES):
        for period,phase in ((200,0),(307,61)):
            n=(frame+phase)%period
            triangle=2*min(n,period-n)-period//2
            value=triangle*scale//period
            raw.extend(value.to_bytes(depth//8,'little',signed=True))
    with wave.open(str(path),'wb') as wav:
        wav.setnchannels(2);wav.setsampwidth(depth//8);wav.setframerate(RATE);wav.writeframes(raw)
    with wave.open(str(path),'rb') as wav:
        require((wav.getnchannels(),wav.getsampwidth(),wav.getframerate(),wav.getnframes())==(2,depth//8,RATE,FRAMES),'authored WAV descriptor differs')
        require(wav.readframes(FRAMES)==raw,'authored WAV samples differ')
    return bytes(raw)

def pilot(binary,tool_directory,work):
    observer_hash=hashed(Path(__file__))
    binary=binary.resolve(strict=True);tool_directory=tool_directory.resolve(strict=True)
    ffmpeg=tool_directory/'ffmpeg';ffprobe=tool_directory/'ffprobe'
    require(hashed(ffmpeg)==FFMPEG_SHA and hashed(ffprobe)==FFPROBE_SHA,'selected audio provider bytes differ')
    require(work.parent.resolve()==work.parent and work.parent.is_dir(),'canonical existing work parent required');work.mkdir(mode=0o700)
    inputs=work/'inputs';inputs.mkdir();outputs=work/'outputs';outputs.mkdir();observed=work/'observed';observed.mkdir()
    env=dict(os.environ,PATH=str(tool_directory)+os.pathsep+os.environ['PATH'],NO_COLOR='1',TERM='dumb')
    require(Path(shutil.which('ffmpeg',path=env['PATH'])).resolve()==ffmpeg and Path(shutil.which('ffprobe',path=env['PATH'])).resolve()==ffprobe,'provider lookup differs')
    events=[];binary_sha=hashed(binary)
    def run(name,argv,code=0):
        started=time.monotonic();result=capture(list(map(str,argv)),inputs,env)
        (observed/(name+'.stdout')).write_bytes(result.stdout);(observed/(name+'.stderr')).write_bytes(result.stderr)
        require(result.returncode==code,'unexpected native exit for '+name)
        events.append({'name':name,'argv':[str(v).replace(str(binary),'{renderflow}').replace(str(tool_directory),'{audio-tools}').replace(str(work),'{work}') for v in argv],
            'exit_code':result.returncode,'elapsed_ms':round((time.monotonic()-started)*1000),'stdout_sha256':sha(result.stdout),'stderr_sha256':sha(result.stderr)})
        return result
    for tool in (ffmpeg,ffprobe):
        version=run(tool.name+'-version',[tool,'-version']).stdout.decode();require(version.startswith(tool.name+' version 8.0.1 '),'provider version differs')
    source24=inputs/'authored-24bit-44100-stereo.wav';source16=inputs/'authored-16bit-44100-stereo.wav'
    originals={24:authored(source24,24),16:authored(source16,16)}
    def config(name,source,targets):
        value={'schema':'renderflow/v2','sources':[{'id':'authored-audio','path':str(source),'format':'wav'}],
            'targets':{'exact':targets},'execution':{'max_parallel':1},'output':{'bundle_root':str(outputs/name)}}
        path=inputs/(name+'.json');put(path,value);return path
    first=config('pcm24-delivery',source24,[{'id':'flac','role':'delivery/flac','format':'flac','preset':'flac_max'},
        {'id':'mp3','role':'delivery/mp3','format':'mp3','preset':'hq_320k'},
        {'id':'aac','role':'delivery/aac-m4a','format':'m4a','preset':'aac_256k'}])
    second=config('pcm16-alac',source16,[{'id':'alac','role':'delivery/alac','format':'m4a_alac','preset':'alac_cd'},
        {'id':'aac-comparison','role':'delivery/aac-m4a','format':'m4a','preset':'aac_256k'}])
    inputs_before=inventory(inputs);results=[]
    for name,path,source,depth,expected in [('pcm24-delivery',first,source24,24,{'flac':'flac','mp3':'mp3','m4a':'aac'}),
                                          ('pcm16-alac',second,source16,16,{'m4a_alac':'alac','m4a':'aac'})]:
        run(name+'-plan',[binary,'graph','plan','--config',path,'--format','json'])
        run(name+'-build',[binary,'build','--config',path])
        root=outputs/name;manifest_path=root/'renderflow-run.json';manifest=json.loads(manifest_path.read_bytes())
        require(manifest['state']=='complete','native audio run not complete')
        rows=[row for row in manifest['artifact_manifest']['artifacts'] if row['lifecycle']=='terminal']
        require(len(rows)==len(expected) and {row['format'] for row in rows}==set(expected)
            and len({row['locator'] for row in rows})==len(rows) and len({row['artifact_id'] for row in rows})==len(rows),'wrong actual audio target set')
        require(len(manifest['artifact_manifest']['outputs'])==len(rows)
            and {Path(p).resolve() for p in manifest['artifact_manifest']['outputs']}=={(root/row['locator'][7:]).resolve() for row in rows},'native audio output membership differs')
        source_rows=[r for r in manifest['artifact_manifest']['artifacts'] if r['lifecycle']=='source']
        require(len(source_rows)==1 and source_rows[0]['format']=='wav' and source_rows[0]['sources']==[]
            and source_rows[0]['digest']['value']==hashed(source) and source_rows[0]['size_bytes']==source.stat().st_size,'native source is detached from authored WAV')
        selected=[b for b in manifest['artifact_forest']['branches'] if b['state']=='selected']
        require(len(selected)==len(rows) and {b['format'] for b in selected}==set(expected),'native selected forest branches differ')
        tools=manifest['toolchain']['selected_tools'];require(len(tools)==1 and tools[0]['id']=='tool.ffmpeg'
            and tools[0]['executable']=='ffmpeg' and tools[0]['version']=='8.0.1','native selected provider differs from pinned PATH tool')
        require(Path(shutil.which('ffmpeg',path=env['PATH'])).resolve()==ffmpeg and hashed(ffmpeg)==FFMPEG_SHA,'native PATH provider bytes changed')
        source_probe=json.loads(run(name+'-source-probe',[ffprobe,'-v','error','-show_streams','-show_format','-of','json',source]).stdout)
        source_stream=source_probe['streams'][0];require(len(source_probe['streams'])==1 and int(source_stream['sample_rate'])==RATE and source_stream['channels']==2
            and source_stream['codec_name']=='pcm_s'+str(depth)+'le' and int(source_stream['bits_per_sample'])==depth
            and Fraction(source_stream['duration_ts'])*Fraction(source_stream['time_base'])*RATE==FRAMES,'original PCM codec/depth/frame extent/rate/channels differ')
        if name=='pcm16-alac':
            require({Path(row['locator']).suffix for row in rows}=={'.m4a'}
                and len({row['metadata']['renderflow.target.identity'] for row in rows})==2,'ALAC/AAC same-container targets collided')
        for row in rows:
            require(row['locator'].startswith('bundle:') and row['validation'] in ('valid','valid_with_warnings') and row['metadata']['renderflow.target.published'] is True,'audio target is not published/validated')
            artifact=root/row['locator'][7:];require(artifact.resolve().is_relative_to(root) and artifact.is_file(),'audio output escaped bundle')
            require(hashed(artifact)==row['digest']['value'] and artifact.stat().st_size==row['size_bytes'],'native artifact digest mismatch')
            fmt=row['format'];label=name+'-'+fmt
            branch=next(b for b in selected if b['format']==fmt)
            requested=next(t for t in json.loads(path.read_bytes())['targets']['exact'] if t['format']==fmt)
            require(branch['preset']==requested['preset'] and branch['role']==requested['role']
                and row['metadata']['renderflow.target.identity']==branch['target_id']
                and row['sources']==[source_rows[0]['artifact_id']] and row['producer']['provider']=='tool.ffmpeg'
                and row['producer']['transform']=='renderflow.strategy-adapter' and row['producer']['capability']=='transform.wav.'+fmt
                and row['metadata']['renderflow.adapter']=='builtin.strategy' and row['metadata']['renderflow.from']=='wav'
                and row['metadata']['renderflow.to']==fmt,'native source/producer/forest/profile binding differs')
            require(len(branch['route']['steps'])==1 and branch['route']['steps'][0]['provider_id']=='tool.ffmpeg'
                and branch['route']['steps'][0]['capability_id']=='transform.wav.'+fmt,'unexpected audio route/provider')
            if fmt=='m4a_alac':require(artifact.suffix=='.m4a','ALAC published suffix is not its physical container')
            probe=json.loads(run(label+'-probe',[ffprobe,'-v','error','-show_streams','-show_format','-show_packets','-of','json',artifact]).stdout)
            require(len(probe['streams'])==1,'unexpected extra output streams');stream=probe['streams'][0]
            require(stream['codec_name']==expected[fmt] and stream['codec_type']=='audio' and int(stream['sample_rate'])==RATE and stream['channels']==2,'actual audio codec/rate/channels differ')
            pcm_format='s24le' if depth==24 else 's16le';decoded=observed/(label+'.'+pcm_format)
            run(label+'-decode',[ffmpeg,'-v','error','-i',artifact,'-map','0:a:0','-c:a','pcm_'+pcm_format,'-f',pcm_format,decoded])
            raw=decoded.read_bytes();frame_bytes=depth//8*2;require(len(raw)%frame_bytes==0,'decoded sample framing differs')
            frame_count=len(raw)//frame_bytes;lossless=fmt in ('flac','m4a_alac')
            raw_stream_duration=Fraction(stream['duration_ts'])*Fraction(stream['time_base']) if 'duration_ts' in stream else Fraction(stream['duration'])
            packets=probe['packets'];require(packets and all(p.get('codec_type')=='audio' for p in packets),'missing audio packet observation')
            base=Fraction(stream['time_base']);require(all('pts' in p and int(p.get('duration',0))>0 for p in packets),'missing/invalid packet clocks')
            starts=[Fraction(p['pts'])*base for p in packets];ends=[Fraction(int(p['pts'])+int(p['duration']))*base for p in packets]
            require(all(starts[i]==ends[i-1] for i in range(1,len(packets))),'packet sequence has overlap/gap/reorder')
            skip=discard=0
            for packet in packets:
                for side in packet.get('side_data_list',[]):
                    if side['side_data_type']=='Skip Samples':
                        require(int(side['skip_samples'])>=0 and int(side['discard_padding'])>=0,'negative priming/padding')
                        skip+=int(side['skip_samples']);discard+=int(side['discard_padding'])
            packet_span=ends[-1]-starts[0]
            presentation=packet_span-Fraction(skip+discard,RATE)
            require(presentation==Fraction(FRAMES,RATE),'declared packet skip/discard does not reconcile authored duration')
            if lossless:
                require(raw==originals[depth] and frame_count==FRAMES,'lossless decoded samples differ')
                require(raw_stream_duration==3 and starts[0]==0 and ends[-1]==3 and skip==discard==0
                    and int(stream['bits_per_raw_sample'])==depth,'lossless presentation/packet/depth preservation differs')
                clock_bound='exact source samples; packet/presentation 0..3s; no priming/padding'
            elif fmt=='mp3':
                require(raw!=originals[depth] and frame_count==FRAMES,'MP3 lossy/gapless sample count differs')
                require(starts[0]==0 and 0<skip<=1152 and 0<=discard<1152
                    and all((end-start)*RATE==1152 for start,end in zip(starts,ends))
                    and 3<=raw_stream_duration<=packet_span
                    and Fraction(stream['start_pts'])*base==Fraction(skip,RATE),'MP3 frame/priming/padding bounds differ')
                clock_bound='1152-sample MP3 packets; leading skip <=1152; trailing discard <1152; gapless decoded/presentation exactly source'
            else:
                require(raw!=originals[depth] and FRAMES<=frame_count<FRAMES+1024,'AAC decoded padded-tail bound differs')
                require(starts[0]==Fraction(-1024,RATE) and ends[-1]==3 and skip==1024 and discard==0
                    and raw_stream_duration==3 and all((end-start)*RATE<=1024 for start,end in zip(starts,ends)),
                    'AAC/M4A priming/presentation bounds differ')
                clock_bound='1024-sample AAC priming; presentation exactly source; decoded tail <1024 samples retained without artificial trim'
            results.append({'case':name,'format':fmt,'codec':stream['codec_name'],'selected_profile':branch['preset'],'native_branch_target_id':branch['target_id'],'native_provider':{'id':tools[0]['id'],'executable':tools[0]['executable'],'version':tools[0]['version']},'source_artifact_id':source_rows[0]['artifact_id'],
                'sample_rate_hz':RATE,'channels':2,'source_bit_depth':depth,'source_wav_sha256':hashed(source),'source_frames':FRAMES,'decoded_frames':frame_count,
                'source_sample_sha256':sha(originals[depth]),'decoded_sample_sha256':sha(raw),'decoded_exactly_equal':raw==originals[depth],
                'lossless_selected':lossless,'raw_stream_duration_seconds':str(raw_stream_duration),'presentation_after_declared_skip_discard_seconds':str(presentation),'codec_clock_bound':clock_bound,'skip_samples':skip,'discard_padding':discard,'packet_begin_seconds':str(min(starts)),
                'packet_end_seconds':str(max(ends)),'packet_count':len(packets),'artifact_sha256':hashed(artifact),'artifact_bytes':artifact.stat().st_size,
                'native_fidelity':row['fidelity'],'native_validation':row['validation'],'native_validation_evidence':row.get('validation_evidence',[]),
                'skip_samples_or_discard_padding':[p['side_data_list'] for p in packets if 'side_data_list' in p]})
    require(inventory(inputs)==inputs_before,'original WAV/configuration bytes changed')
    require(hashed(binary)==binary_sha and hashed(ffmpeg)==FFMPEG_SHA and hashed(ffprobe)==FFPROBE_SHA,'selected executable bytes changed')
    require(hashed(Path(__file__))==observer_hash,'selected audio observer changed')
    report={'schema_version':'renderflow.audio-delivery-candidate-proof/v1','observed_at_utc':datetime.now(timezone.utc).isoformat(),
        'observer_sha256':observer_hash,'binary_sha256':binary_sha,'ffmpeg_sha256':FFMPEG_SHA,'ffprobe_sha256':FFPROBE_SHA,
        'source_duration_seconds':'3','sample_rate_hz':RATE,'channels':2,'events':events,'results':results,
        'input_inventory':inputs_before,'output_inventory':inventory(outputs),'observation_inventory':inventory(observed),
        'mastering_or_loudness_approval':False,'perceptual_or_physical_playback_approval':False,'native_platform_or_full_provider_matrix_qualified':False,
        'publisher_authenticated':False,'remote_publication_performed':False}
    put(work/'report.json',report);return report

if __name__=='__main__':
    parser=argparse.ArgumentParser(description=__doc__);parser.add_argument('--binary',type=Path,required=True);parser.add_argument('--audio-tool-directory',type=Path,required=True);parser.add_argument('--work-directory',type=Path,required=True)
    args=parser.parse_args();report=pilot(args.binary,args.audio_tool_directory,args.work_directory.absolute())
    print(json.dumps({'state':'selected_audio_delivery_checks_passed','binary_sha256':report['binary_sha256'],'results':len(report['results'])},sort_keys=True))
