#!/usr/bin/env python3
"""Observe the configured native validator through the actual canonical CLI.

Reuses the existing authored-audio observer helpers. No tool preparation,
publication, real content, mastering or general matrix qualification.
"""
from __future__ import annotations
import argparse
from fractions import Fraction
import hashlib
import importlib.util
import json
import os
from pathlib import Path
import shutil
import sys
from datetime import datetime,timezone
sys.dont_write_bytecode=True
NATIVE='validator.audio.pinned_pcm_delivery'

def main(binary,tools,slim,work,helper):
    spec=importlib.util.spec_from_file_location('delivery_observer',helper);base=importlib.util.module_from_spec(spec);spec.loader.exec_module(base)
    require=base.require;hashed=base.hashed;put=base.put;inventory=base.inventory
    observer_sha=hashed(Path(__file__));helper_sha=hashed(helper);binary=binary.resolve(strict=True);binary_sha=hashed(binary)
    tools=tools.resolve(strict=True);slim=slim.resolve(strict=True)
    require(hashed(tools/'ffmpeg')==base.FFMPEG_SHA and hashed(tools/'ffprobe')==base.FFPROBE_SHA,'wrong selected audio tools')
    require(hashed(slim/'ffmpeg')=='098c4ba3d9d7ca067bc9bfa004a33b5b91cf0913fa9380bcda7f3975c214380b'
        and hashed(slim/'ffprobe')=='31054f1d68b781089f98f594bb304cf5c9ae96dadafafdd021719ab4101a79be','wrong preserved slim control')
    verifier_subjects={kind:{name:{'sha256':hashed(directory/name),'bytes':(directory/name).stat().st_size}
        for name in ('ffmpeg','ffprobe')} for kind,directory in (('audio',tools),('preserved_slim',slim))}
    require(work.parent.resolve()==work.parent and work.parent.is_dir(),'canonical existing work parent required');work.mkdir(mode=0o700)
    inputs=work/'inputs';outputs=work/'outputs';logs=work/'observed'
    for p in (inputs,outputs,logs):p.mkdir()
    env=dict(os.environ,PATH=str(tools)+os.pathsep+os.environ['PATH'],LC_ALL='C',NO_COLOR='1',TERM='dumb')
    events=[];summaries=[];feature_cache={};limitations=[]
    def run(name,argv,code=0):
        result=base.capture(list(map(str,argv)),inputs,env,timeout=60)
        (logs/(name+'.stdout')).write_bytes(result.stdout);(logs/(name+'.stderr')).write_bytes(result.stderr)
        require(result.returncode==code,'unexpected native exit '+name)
        events.append({'name':name,'argv':[str(v).replace(str(binary),'{renderflow}').replace(str(work),'{work}').replace(str(tools),'{audio-tools}').replace(str(slim),'{slim-verifiers}') for v in argv],
            'exit_code':result.returncode,'stdout_sha256':hashlib.sha256(result.stdout).hexdigest(),'stderr_sha256':hashlib.sha256(result.stderr).hexdigest()})
        return result
    def policy(directory):return {'profile':'pcm-delivery-ffmpeg-8.0.1-v1','ffmpeg':{'path':str(directory/'ffmpeg'),'sha256':hashed(directory/'ffmpeg')},'ffprobe':{'path':str(directory/'ffprobe'),'sha256':hashed(directory/'ffprobe')}}
    selected=policy(tools);slim_policy=policy(slim)
    sources={};pcm={}
    for depth in (16,24,32):
        path=inputs/f'authored-{depth}.wav';pcm[depth]=base.authored(path,depth);sources[depth]=path
    source_before={str(p.name):{'bytes':p.stat().st_size,'sha256':hashed(p)} for p in sources.values()}
    def targets(*items):return [{'id':fmt,'role':'delivery/'+fmt,'format':fmt,**({'preset':preset} if preset else {})} for fmt,preset in items]
    def configured(name,depth,selection,validation=None,execution=None,chosen=selected):
        value={'schema':'renderflow/v2','sources':[{'id':'native-audio','path':str(sources[depth]),'format':'wav'}],
            'targets':{'exact':selection},'execution':{'max_parallel':1,'validation':dict(validation or {})},'output':{'bundle_root':str(outputs/name)}}
        if chosen is not None:value['execution']['validation']['audio']=chosen
        value['execution'].update(execution or {});path=inputs/(name+'.json');put(path,value);return path
    def build(name,config,state='complete'):
        result=run(name,[binary,'build','--config',config],0 if state=='complete' else 1)
        root=Path(json.loads(config.read_bytes())['output']['bundle_root']);path=root/'renderflow-run.json';manifest=json.loads(path.read_bytes())
        require(manifest['state']==state,'wrong structured state '+name)
        if state!='complete':
            tail=f"Error: renderflow execution finished with state {state.title()}; inspect '{path}' for structured evidence\n".encode()
            require(result.stderr.endswith(tail),'different owning native refusal '+name)
        rows=[r for r in manifest['artifact_manifest']['artifacts'] if r['lifecycle']=='terminal']
        summaries.append({'name':name,'state':state,'manifest_sha256':hashed(path),'published':len(manifest['artifact_manifest']['outputs']),
            'targets':[{'format':r['format'],'artifact_id':r['artifact_id'],'validation':r['validation'],'fidelity':r['fidelity'],'published':r['metadata'].get('renderflow.target.published'),
                'validators':r.get('validation_evidence',[]),'native':r['metadata'].get('renderflow.audio.native_validation'),'feature_admission':r['metadata'].get('renderflow.audio.native_feature_admission')} for r in rows]})
        return root,manifest,rows
    def native(row,state='valid',code=None):
        records=[v for v in row['validation_evidence'] if v['validator_id']==NATIVE]
        require(len(records)==1 and records[0]['state']==state,'native check missing/duplicated/wrong state')
        if code:require([d['code'] for d in records[0]['diagnostics']]==[code],'different owning native code')
        return row['metadata'].get('renderflow.audio.native_validation')
    def feature_observation(admission,directory):
        require(len(admission['queries'])==6,'wrong feature query closure')
        for q in admission['queries']:
            require(q['capture_complete'] and q['recognized'],'incomplete feature query')
            tool=directory/('ffmpeg' if q['tool_id']=='tool.ffmpeg' else 'ffprobe');key=(hashed(tool),tuple(q['argv']))
            if key not in feature_cache:
                label='feature-'+str(len(feature_cache));r=run(label,[tool,*q['argv']]);feature_cache[key]=(hashlib.sha256(r.stdout).hexdigest(),hashlib.sha256(r.stderr).hexdigest())
            require(feature_cache[key]==(q['stdout_sha256'],q['stderr_sha256']),'native feature witness differs from selected tools')
    def positive(name,depth,selection,changes=False):
        config=configured(name,depth,selection);root,m,rows=build(name,config)
        require(len(rows)==len(selection) and len({r['artifact_id'] for r in rows})==len(rows) and len({r['locator'] for r in rows})==len(rows),'terminal identities collided')
        require({str(root/r['locator'][7:]) for r in rows}==set(m['artifact_manifest']['outputs']),'output list differs')
        source=next(r for r in m['artifact_manifest']['artifacts'] if r['lifecycle']=='source');require(source['digest']['value']==hashed(sources[depth]),'source detached')
        for row in rows:
            n=native(row);require(row['validation']=='valid' and row['metadata']['renderflow.target.published'] is True,'native output not accepted')
            branch=next(b for b in m['artifact_forest']['branches'] if b['target_id']==row['metadata']['renderflow.target.identity'])
            step=next(s for s in m['steps'] if s['step_id']==n['producing_step_id'])
            require(n['target_identity']==branch['target_id'] and n['requested_audio_profile']==branch['preset'] and n['artifact_id']==row['artifact_id']
                and n['artifact_sha256']==row['digest']['value'] and n['source_artifact_id']==source['artifact_id'] and n['source_sha256']==source['digest']['value']
                and n['producing_step_configuration_sha256']==step['configuration_digest']['value'],'target/source/profile/step binding detached')
            require(step['fidelity']==row['fidelity']==n['fidelity'] and row['sources']==[source['artifact_id']],'native fidelity or lineage differs')
            require(n['verifier_ffmpeg']['sha256']==base.FFMPEG_SHA and n['verifier_ffprobe']['sha256']==base.FFPROBE_SHA
                and n['encoded_bitrate_or_compression_attested'] is False and n['encoder_executable_hash_attested'] is False
                and n['native_library_or_os_sandbox_attested'] is False and n['mastering_playback_or_content_approval'] is False,'attestation authority changed')
            admission=n['feature_admission'];require(admission['admitted'] and all(v['matched'] for v in admission['required']),'missing native prerequisites');feature_observation(admission,tools)
            require(all(v['validator_id']!='validator.core.declared_signature' for v in row['validation_evidence']),'default deep check fell back to signature')
            artifact=root/row['locator'][7:];require(hashed(artifact)==row['digest']['value'],'output digest differs')
            label=name+'-'+row['format'];probe=json.loads(run(label+'-probe',[tools/'ffprobe','-v','error','-show_streams','-show_packets','-of','json',artifact]).stdout)
            require(len(probe['streams'])==1,'extra media stream');s=probe['streams'][0];clock=n['observed_clock']
            require(s['codec_name']=={'flac':'flac','m4a_alac':'alac','m4a':'aac','mp3':'mp3'}[row['format']], 'independent codec differs')
            require(int(s['sample_rate'])==clock['rate_hz'] and s['channels']==clock['channels'],'independent layout differs')
            tick=Fraction(s['time_base'])*int(s['sample_rate']);packets=probe['packets']
            require(Fraction(s['duration_ts'])*tick==clock['stream_duration_samples'] and Fraction(packets[0]['pts'])*tick==clock['packet_start_sample']
                and Fraction(int(packets[-1]['pts'])+int(packets[-1]['duration']))*tick==clock['packet_end_sample'],'independent packet/stream clock differs')
            skip=sum(int(d.get('skip_samples',0)) for packet in packets for d in packet.get('side_data_list',[]) if d['side_data_type']=='Skip Samples')
            discard=sum(int(d.get('discard_padding',0)) for packet in packets for d in packet.get('side_data_list',[]) if d['side_data_type']=='Skip Samples')
            require(skip==clock['skip_samples'] and discard==clock['discard_padding'] and clock['packet_end_sample']-clock['packet_start_sample']-skip-discard==clock['presentation_samples'],'independent presentation reconciliation differs')
            for index,packet in enumerate(packets):
                span=Fraction(packet['duration'])*tick
                require(packet['codec_type']=='audio' and span>0,'independent packet shape differs')
                if index:require(Fraction(packet['pts'])*tick==(Fraction(packets[index-1]['pts'])+int(packets[index-1]['duration']))*tick,'independent packet sequence differs')
                if row['format']=='mp3':require(span==1152,'independent MPEG1 frame differs')
                if row['format']=='m4a':require(span<=1024 and (index+1==len(packets) or span==1024),'independent AAC interior frame differs')
            bits=n['decoded_precision_bits'];raw=logs/(label+'.pcm');run(label+'-decode',[tools/'ffmpeg','-v','error','-i',artifact,'-map','0:a:0','-c:a',f'pcm_s{bits}le','-f',f's{bits}le',raw])
            require(raw.stat().st_size%(bits//8*s['channels'])==0 and hashed(raw)==n['decoded_pcm_sha256'] and raw.stat().st_size//(bits//8*s['channels'])==n['decoded_frames'],'independent decoded observation differs')
            require(n['source_pcm']['sample_sha256']==hashlib.sha256(pcm[depth]).hexdigest(),'original PCM binding differs')
            lossless=row['format'] in ('flac','m4a_alac') and not changes
            require(row['fidelity']==('lossless' if lossless else 'lossy'),'incorrect artifact-specific fidelity')
            if lossless:require(raw.read_bytes()==pcm[depth] and n['decoded_samples_equal_original'] and not n['profile_changes_pcm_layout'],'native lossless claim is not exact PCM')
            if changes:require(n['profile_changes_pcm_layout'] and not n['decoded_samples_equal_original'],'declared resample became source preserving')
        return config,root,m,rows
    positive('native24',24,targets(('flac','flac_max'),('mp3','hq_320k'),('m4a','aac_256k')))
    _,_,_,same=positive('native16',16,targets(('m4a_alac','alac_cd'),('m4a','aac_256k')))
    require({Path(r['locator']).suffix for r in same}=={'.m4a'},'same-container AAC/ALAC suffix changed')
    positive('declared-resample',24,targets(('flac','flac_broadcast')),changes=True)
    # Existing encoder does not force ALAC CD precision on a24-bit source.
    config=configured('unmet-cd-precision',24,targets(('m4a_alac','alac_cd')));_,m,rows=build('unmet-cd-precision',config,'failed')
    require(not m['artifact_manifest']['outputs'],'profile mismatch published');native(rows[0],'invalid','audio.native.profile.layout')
    refused=rows[0];require(refused['locator'].startswith('artifact-store:'),'unaccepted CD artifact has no CAS locator')
    cas=outputs/'.renderflow'/'artifacts'/refused['locator'][15:];require(hashed(cas)==refused['digest']['value'],'retained refused CAS payload differs')
    observed=json.loads(run('unmet-cd-precision-probe',[tools/'ffprobe','-v','error','-show_streams','-of','json',cas]).stdout)['streams'][0]
    require(observed['codec_name']=='alac' and observed['bits_per_raw_sample']=='24','existing CD precision limitation differs')
    limitations.append({'case':'unmet-cd-precision','requested_precision_bits':16,'observed_precision_bits':24,'codec':'alac','native_state':'invalid','native_code':'audio.native.profile.layout','accepted_publication':False})
    config=configured('legacy-default',24,targets(('flac','flac_max')),chosen=None);_,_,rows=build('legacy-default',config)
    require(rows[0]['validation']=='valid_with_warnings' and rows[0]['fidelity']=='lossy' and all(v['validator_id']!=NATIVE for v in rows[0]['validation_evidence']),'absent-policy behavior changed')
    config=configured('explicit-signature',24,targets(('flac','flac_max')),{'validators':['validator.core.declared_signature']});_,_,rows=build('explicit-signature',config)
    require(rows[0]['validation']=='valid_with_warnings','explicit signature warning disappeared');native(rows[0])
    config=configured('explicit-native-id',24,targets(('flac','flac_max')),{'validators':[NATIVE]});_,_,rows=build('explicit-native-id',config);native(rows[0]);require(rows[0]['validation']=='valid','contextual native id was not delegated once')
    config=configured('unknown-custom',24,targets(('flac','flac_max')),{'validators':['validator.custom.unavailable']});_,m,rows=build('unknown-custom',config,'failed');native(rows[0])
    require(not m['artifact_manifest']['outputs'] and any(v['validator_id']=='validator.custom.unavailable' and v['state']=='unavailable' for v in rows[0]['validation_evidence']),'custom failure bypassed')
    wrong=json.loads(json.dumps(selected));wrong['ffprobe']['sha256']='0'*64
    config=configured('wrong-verifier-digest',24,targets(('flac','flac_max')),{'required':False,'allow_unavailable':True},chosen=wrong);_,m,rows=build('wrong-verifier-digest',config,'failed');native(rows[0],'invalid','audio.native.verifier_digest');require(not m['artifact_manifest']['outputs'],'wrong verifier published')
    config=configured('denied-verifier',24,targets(('flac','flac_max')),execution={'tools':{'deny':['tool.ffprobe']}});_,m,rows=build('denied-verifier',config,'failed');native(rows[0],'unavailable','audio.native.verifier_policy_denied');require(not m['artifact_manifest']['outputs'],'denied verifier published')
    config=configured('slim-prerequisites',24,targets(('flac','flac_max')),chosen=slim_policy);_,m,rows=build('slim-prerequisites',config,'failed');native(rows[0],'unavailable','audio.native.feature_missing')
    admission=rows[0]['metadata']['renderflow.audio.native_feature_admission'];require(not admission['admitted'] and any(not v['matched'] for v in admission['required']),'missing features were invented');feature_observation(admission,slim);require(not m['artifact_manifest']['outputs'],'unsupported verifier published')
    config=configured('unsupported-precision',32,targets(('wav',None)));_,m,rows=build('unsupported-precision',config,'failed');native(rows[0],'unavailable','audio.native.source_precision_unsupported');require(not m['artifact_manifest']['outputs'],'unsupported PCM32 qualified')
    config=configured('loss-class-gate',24,targets(('flac','flac_max'),('mp3','hq_320k')),execution={'reject_loss_classes':['lossy']});_,m,rows=build('loss-class-gate',config,'partial')
    by_format={r['format']:r for r in rows};require(by_format['flac']['fidelity']=='lossless' and by_format['flac']['metadata']['renderflow.target.published'] is True
        and by_format['mp3']['fidelity']=='lossy' and by_format['mp3']['metadata']['renderflow.target.published'] is False and len(m['artifact_manifest']['outputs'])==1,'observed fidelity did not drive existing loss gate')
    # Identical config/cache is revalidated after private verifier disappearance.
    copied=work/'copied-verifiers';copied.mkdir()
    for name in ('ffmpeg','ffprobe'):shutil.copy2(tools/name,copied/name)
    copied_policy=policy(copied);config=configured('cache-native-required',24,targets(('flac','flac_max')),{'required':False,'allow_unavailable':True},chosen=copied_policy)
    root,m,rows=build('cache-positive',config);native(rows[0]);require(rows[0]['fidelity']=='lossless','positive cache baseline not qualified')
    put(logs/'cache-positive-manifest.json',m);payload=root/rows[0]['locator'][7:];before=hashed(payload);(copied/'ffprobe').unlink()
    _,failed,rows=build('cache-revalidate-missing',config,'failed');native(rows[0],'unavailable','audio.native.verifier_missing')
    require(not failed['artifact_manifest']['outputs'] and any(s['state']=='reused' for s in failed['steps']) and hashed(payload)==before
        and rows[0]['metadata']['renderflow.target.published'] is False,'cached proof bypassed fresh required validation or changed old payload')
    require({str(p.name):{'bytes':p.stat().st_size,'sha256':hashed(p)} for p in sources.values()}==source_before,'original WAV changed')
    require(hashed(binary)==binary_sha and hashed(helper)==helper_sha and hashed(Path(__file__))==observer_sha,'selected source/executable changed')
    require(hashed(tools/'ffmpeg')==base.FFMPEG_SHA and hashed(tools/'ffprobe')==base.FFPROBE_SHA,'preserved verifier changed')
    require({kind:{name:{'sha256':hashed(directory/name),'bytes':(directory/name).stat().st_size}
        for name in ('ffmpeg','ffprobe')} for kind,directory in (('audio',tools),('preserved_slim',slim))}==verifier_subjects,'selected verifier subject changed')
    report={'schema_version':'renderflow.native-audio-cli-proof/v1','observed_at_utc':datetime.now(timezone.utc).isoformat(),'binary_sha256':binary_sha,'observer_sha256':observer_sha,'existing_audio_helper_sha256':helper_sha,
        'verifier_subjects':verifier_subjects,
        'events':events,'cases':summaries,'retained_encoder_limitations':limitations,'source_inventory':source_before,'input_inventory':inventory(inputs),'output_inventory':inventory(outputs),'observation_inventory':inventory(logs),
        'scope':'Direct selected-WAV native structural/PCM/clock validation only; existing ALAC CD precision encoder limitation retained as an Invalid refusal.',
        'encoder_executable_hash_attested':False,'native_platform_matrix_qualified':False,'mastering_loudness_or_creative_approval':False,'remote_publication_performed':False}
    put(work/'report.json',report);print(json.dumps({'state':'native_audio_cli_checks_passed','cases':len(summaries),'commands':len(events),'binary_sha256':binary_sha},sort_keys=True))

if __name__=='__main__':
    parser=argparse.ArgumentParser(description=__doc__)
    for name in ('binary','audio-tool-directory','slim-verifier-directory','work-directory'):parser.add_argument('--'+name,type=Path,required=True)
    parser.add_argument('--observer-helper',type=Path,default=Path(__file__).with_name('check_audio_delivery_candidate.py'))
    args=parser.parse_args();main(args.binary,args.audio_tool_directory,args.slim_verifier_directory,args.work_directory.absolute(),args.observer_helper.resolve(strict=True))
