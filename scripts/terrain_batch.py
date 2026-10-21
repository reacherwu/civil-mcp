# Real-terrain route test: imports each DEM / point cloud with import_terrain and runs
# generate_route_alternatives between points at 15 % and 85 % of the surface extent.
# Data: see docs/terrain-tests.md for download commands. Usage: DEM_DIR=/tmp/dem/ python scripts/terrain_batch.py [site ...]
import json,sys,time
import os;sys.path.insert(0,os.path.dirname(__file__));from mcp_driver import M
asm=[json.loads(l)['args'] for l in open('demo-out/run-log.jsonl') if json.loads(l)['tool']=='create_assembly'][0]
def run(name,path,imp={},route={}):
  m=M();m.call('open_project',{'dir':f'/tmp/site-out/{name}'})
  m.call('set_design_basis',{"actor":"engineer:TEST","basis":{"project":name,"engineer_of_record":"TEST","standard":"US_AASHTO_2018","design_speed_kmh":50.0,"e_max":0.08,"f_max":0.18,"max_grade_pct":10.0,"normal_crown":0.02,"lane_width_m":3.5}})
  t=time.time();r=m.call('import_terrain',dict({'path':path},**imp))
  if not r: return
  r=r['result'];b=r['bbox']
  out={'site':name,'fmt':r['format'],'epsg':r['crs']['source_epsg'],'crs':r['crs']['source_kind'],'target':r['crs']['target_epsg'],'pts':r['kept_points'],'cell':r['cell_m'] and round(r['cell_m'],2),'z':[round(v,1) for v in r['z_range']],'import_s':round(time.time()-t,1)}
  m.call('create_assembly',asm)
  s0=[b[0]+.15*(b[2]-b[0]),b[1]+.15*(b[3]-b[1])];s1=[b[0]+.85*(b[2]-b[0]),b[1]+.85*(b[3]-b[1])]
  a=dict({"start":s0,"end":s1,"cell_m":max(10.0,r['cell_m'] or 0),"radius_pref_m":120.0,"spiral_m":30.0,"simplify_tol_m":15.0,"vpi_spacing_m":120.0,"interval_m":20.0,"hairpin_min_radius_m":30.0},**route)
  t=time.time();m.i+=1
  m.p.stdin.write(json.dumps({"jsonrpc":"2.0","id":m.i,"method":"tools/call","params":{"name":"generate_route_alternatives","arguments":a}})+"\n");m.p.stdin.flush()
  x=json.loads(m.p.stdout.readline())['result'];c=x['content'][0]['text'];out['route_s']=round(time.time()-t,1)
  if x.get('isError'): out['result']=c
  else:
    rr=json.loads(c)['result'];out['rec']=rr['recommendation']['recommended'];out['alts']=[]
    for al in rr['alternatives']:
      e=al['evaluation'];out['alts'].append({k:(round(e[k],1) if isinstance(e.get(k),float) else e.get(k)) for k in ['error','length_m','max_design_grade_pct','min_radius_m','cut_m3','fill_m3','max_fill_height_m','max_cut_height_m','max_wall_height_m','design_exceptions'] if k in e}|{'name':al['name'],'fails':[c[:40] for c in e.get('failed_checks',[]) if not c.startswith('SE-')],'n_se_fails':sum(c.startswith('SE-') for c in e.get('failed_checks',[])),'overlap':round(al.get('overlap_with_earlier_pct',0))})
  print(json.dumps(out));sys.stdout.flush()
D=os.environ.get('DEM_DIR','/tmp/dem/')
sites={'boulder':(D+'boulder.tif',{'step':2}),'paloalto':(D+'paloalto.tif',{'step':2}),'paloalto_stateplane_ft':(D+'paloalto_sp.tif',{'step':2}),'kansas':(D+'kansas.tif',{'step':2}),'iowa':(D+'iowa.tif',{'step':2}),'wv':(D+'wv.tif',{'step':2}),'wv10':(D+'wv.tif',{'step':1}),'boulder10':(D+'boulder.tif',{'step':1}),
 'autzen_laz':(D+'autzen.laz',{'step':100,'max_points':80000}),'uk_lake_district':(D+'uk.tif',{'crop':[484383,6031611,488383,6035611]}),'au_blue_mtns':(D+'au.tif',{'crop':[248708,6264749,252708,6268749]})}
for n in (sys.argv[1:] or sites):
  p,i=sites[n];run(n,p,i)
