"""Retain the exact vehicle_Kimchi atlas on eye-bone triangles of the green variant.

No pixels, positions, normals, UVs, weights, animation or triangle winding change.
The owner-requested variant adds one material draw over a disjoint index subset.
"""
import argparse,copy,importlib.util,json,os,struct
from pathlib import Path
import blake3
def read(p):return json.loads(p.read_bytes())
def put(p,b):
 p.parent.mkdir(parents=True,exist_ok=True);t=p.with_name(p.name+'.next');t.write_bytes(b);t.replace(p)
def write(p,d):put(p,(json.dumps(d,ensure_ascii=False,indent=2)+'\n').encode())
def glb(p):
 b=p.read_bytes();n=struct.unpack_from('<I',b,12)[0];return json.loads(b[20:20+n]),b[28+n:]
def pack(d,b):
 j=json.dumps(d,ensure_ascii=False,separators=(',',':')).encode();j+=b' '*(-len(j)%4);b+=b'\0'*(-len(b)%4)
 return b'glTF'+struct.pack('<IIII',2,28+len(j)+len(b),len(j),0x4e4f534a)+j+struct.pack('<II',len(b),0x004e4942)+b
def main():
 ap=argparse.ArgumentParser();ap.add_argument('--work',type=Path,required=True);ap.add_argument('--stage',type=Path,required=True);a=ap.parse_args();root=a.stage.resolve();assert root.is_relative_to(Path('work').resolve())
 source=a.work/'recovery/candidate/models/npc/npc_kimchi.glb';d,binary=glb(source);before_binary=binary
 rel='characters/npcs/fusion_kimchi/fusion_kimchi.glb';out=root/rel
 donor=root/'characters/player/items/vehicle/vehicle_kimchi/models/vehicle_kimchi/model.glb';vehicle,_=glb(donor)
 def accessor(i):
  x=d['accessors'][i];v=d['bufferViews'][x['bufferView']];n={'SCALAR':1,'VEC2':2,'VEC3':3,'VEC4':4}[x['type']];fmt={5126:'f',5123:'H',5125:'I'}[x['componentType']];values=struct.unpack_from('<'+fmt*(x['count']*n),binary,v.get('byteOffset',0)+x.get('byteOffset',0));return [values[i:i+n] for i in range(0,len(values),n)]
 node=next(n for n in d['nodes'] if n.get('skin') is not None);mesh=d['meshes'][node['mesh']];assert len(mesh['primitives'])==1
 prim=mesh['primitives'][0];skin=d['skins'][node['skin']];eyes={i for i,n in enumerate(skin['joints']) if d['nodes'][n]['name'] in ['eye01','eye02']};assert len(eyes)==2
 joints=accessor(prim['attributes']['JOINTS_0']);weights=accessor(prim['attributes']['WEIGHTS_0'])
 indices=[x[0] for x in accessor(prim['indices'])];body=[];face=[]
 # Preserve complete disconnected eye surfaces, including their partly weighted rims.
 parents=list(range(len(joints)))
 def component(i):
  while parents[i]!=i:i=parents[i]
  return i
 for i in range(0,len(indices),3):
  x,y,z=indices[i:i+3];parents[component(x)]=component(y);parents[component(z)]=component(y)
 eye_components={component(i) for i,(js,ws) in enumerate(zip(joints,weights)) if any(j in eyes and w>0 for j,w in zip(js,ws))}
 assert len(eye_components)==4,'review changed eye topology before publishing'
 eye_vertices=[component(i) in eye_components for i in range(len(joints))]
 assert sum(eye_vertices)==60
 for i in range(0,len(indices),3):
  tri=indices[i:i+3];flags=[eye_vertices[v] for v in tri];assert all(flags) or not any(flags),'cross-boundary triangle'
  (face if all(flags) else body).extend(tri)
 assert face and body and len(face)+len(body)==len(indices)
 def index_accessor(values):
  nonlocal binary
  binary+=b'\0'*(-len(binary)%4);offset=len(binary);raw=struct.pack('<'+'I'*len(values),*values);binary+=raw;view=len(d['bufferViews']);d['bufferViews'].append(dict(buffer=0,byteOffset=offset,byteLength=len(raw),target=34963));i=len(d['accessors']);d['accessors'].append(dict(bufferView=view,componentType=5125,count=len(values),type='SCALAR'));return i
 body_material=d['materials'][prim['material']];face_material=copy.deepcopy(body_material);face_material['name']='fusion_kimchi-eyes';face_material['extras']['ffone']['name']='fusion_kimchi-eyes'
 donor_binding=next(x for m in vehicle['materials'] for x in m['extras']['ffone']['textureBindings'] if x['slot']=='_MainTex' and x.get('uri') and 'vehicle_kimchi' in x['uri'].lower())
 binding=copy.deepcopy(donor_binding)
 donor_texture=copy.deepcopy(vehicle['textures'][binding['texture']]);donor_image=copy.deepcopy(vehicle['images'][donor_texture['source']]);donor_sampler=copy.deepcopy(vehicle['samplers'][donor_texture['sampler']])
 spec=importlib.util.spec_from_file_location('closure',Path(__file__).with_name('close-npc-native-textures.py'));c=importlib.util.module_from_spec(spec);spec.loader.exec_module(c)
 redirects={}
 fingerprint=c.sha(repr(c.signature(binding)).encode())[:12]
 shared=root/f'textures/shared/vehicle_kimchi_{fingerprint}.png'
 for i,level in enumerate(binding['mipLevels']):
  ref=shared if i==0 else shared.with_suffix('.mips')/f'mip-{i:02}.png';payload=(donor.parent/level['uri']).read_bytes();assert c.sha(payload)==level['pngSha256']
  if ref.exists():assert ref.read_bytes()==payload
  else:put(ref,payload)
  redirects[level['uri']]=os.path.relpath(ref,out.parent).replace('\\','/')
 for data in [binding,donor_image]:
  for obj in c.walk(data):
   for k,v in list(obj.items()):
    if isinstance(v,str) and v in redirects:obj[k]=redirects[v]
 donor_texture['source']=len(d['images']);d['images'].append(donor_image);donor_texture['sampler']=len(d['samplers']);d['samplers'].append(donor_sampler);binding['texture']=len(d['textures']);d['textures'].append(donor_texture)
 face_material['extras']['ffone']['textureBindings']=[binding if x['slot']=='_MainTex' else x for x in face_material['extras']['ffone']['textureBindings']]
 face_material['pbrMetallicRoughness']['baseColorTexture']={'index':binding['texture']}
 face_prim=copy.deepcopy(prim);face_prim['indices']=index_accessor(face);face_prim['material']=len(d['materials']);d['materials'].append(face_material);face_prim['extras']={'materialSlot':'fusion_kimchi-eyes','npcTableTextureWritable':False};prim['indices']=index_accessor(body);prim.setdefault('extras',{})['npcTableTextureWritable']=True;mesh['primitives'].append(face_prim)
 # Reuse the ordinary model's certified shared texture closure at the same depth.
 original_doc=c.glb(root/'characters/npcs/npc_kimchi/npc_kimchi.glb')
 relocated=set()
 for material in d['materials']:
  for tb in material['extras']['ffone']['textureBindings']:
   if tb is binding or tb.get('sourceName')==binding.get('sourceName'):continue
   if not tb.get('mipLevels'):continue
   old_binding=next(x for x in c.bindings(original_doc) if c.signature(x)==c.signature(tb))
   for level,old_level in zip(tb['mipLevels'],old_binding['mipLevels']):
    if level['uri'] in relocated:continue
    old=level['uri'];target=(out.parent/old_level['uri']).resolve();assert target.is_relative_to(root);assert target.read_bytes()==(source.parent/old).read_bytes()
    relocated.add(old_level['uri'])
    for obj in c.walk(d):
     for k,v in list(obj.items()):
      if isinstance(v,str) and v==old:obj[k]=old_level['uri']
 # The new root is a native appearance identity; node indices/animation targets stay exact.
 old_name=d['nodes'][0]['name'];assert old_name=='npc_kimchi';d['nodes'][0]['name']='fusion_kimchi'
 for obj in c.walk(d.get('asset',{})):
  for k,v in list(obj.items()):
   if v==old_name:obj[k]='fusion_kimchi'
 d['buffers'][0]['byteLength']=len(binary);payload=pack(d,binary);put(out,payload)
 registry=read(root/'_runtime/characters.json');original=next(m for m in registry['models'] if m['logicalName']=='npc_kimchi');new=copy.deepcopy(original);new.update(id='npc/fusion_kimchi',logicalName='fusion_kimchi',glb=rel,glbBlake3=blake3.blake3(payload).hexdigest(),legacyAliases=['npc_3460_kimchi'])
 original['legacyAliases']=[x for x in original.get('legacyAliases',[]) if x!='npc_3460_kimchi'];registry['models']=[m for m in registry['models'] if m['id']!=new['id']]+[new];write(root/'_runtime/characters.json',registry)
 report=dict(path=rel,bodyTriangles=len(body)//3,eyeTriangles=len(face)//3,eyeVertices=sum(eye_vertices),source=str(source),donor=donor.relative_to(root).as_posix(),preservedOriginalBinaryPrefix=binary[:len(before_binary)]==before_binary,sha256=c.sha(payload));write(a.work/'kimchi-eyes-repair.json',report);print(report)
if __name__=='__main__':main()
