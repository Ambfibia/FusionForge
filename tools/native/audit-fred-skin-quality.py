"""Independently audit Fred skin weights against scoped raw Mesh evidence."""
import argparse,json,struct,base64,pathlib,hashlib
p=argparse.ArgumentParser(description=__doc__)
p.add_argument("--evidence-root",type=pathlib.Path,required=True)
p.add_argument("--before",type=pathlib.Path,required=True)
p.add_argument("--after",type=pathlib.Path,required=True)
a=p.parse_args()
root=a.evidence_root; raw=json.loads((root/'mesh.evidence.json').read_bytes())['object']['value']['m_CompressedMesh']
def unpack(p):
 data=int.from_bytes(base64.b64decode(p['m_Data']['base64']),'little'); bits=p['m_BitSize']; return [(data>>(i*bits))&((1<<bits)-1) for i in range(p['m_NumItems'])]
weights=unpack(raw['m_Weights']); bones=unpack(raw['m_BoneIndices']); wc=bc=0; rows=[]; indices=[]
for i in range(1880):
 w=[]; ids=[]; total=0
 for slot in range(4):
  if total>=31: break
  v=31-total if slot==3 else weights[wc]
  if slot!=3: wc+=1
  assert v<=31-total
  w.append(v/31); ids.append(bones[bc]); bc+=1; total+=v
 rows.append(w+[0]*(4-len(w))); indices.append(ids+[0]*(4-len(ids)))
assert wc==len(weights) and bc==len(bones)
source=a.before.read_bytes(); n=struct.unpack_from('<I',source,12)[0]; j=json.loads(source[20:20+n]); p=j['meshes'][0]['primitives'][0]
def accessor(b,key,fmt,size):
 a=j['accessors'][p['attributes'][key]]; v=j['bufferViews'][a['bufferView']]; off=28+n+v.get('byteOffset',0)+a.get('byteOffset',0); return [struct.unpack_from(fmt,b,off+i*v.get('byteStride',size)) for i in range(a['count'])]
original=accessor(source,'WEIGHTS_0','<4f',16)
assert max(abs(a-b) for row,expected in zip(original,rows) for a,b in zip(row,expected))<3e-8
assert accessor(source,'JOINTS_0','<4H',8)==[tuple(r) for r in indices]
fixed=a.after.read_bytes(); actual=accessor(fixed,'WEIGHTS_0','<4f',16)
for w,a in zip(rows,actual):
 total=w[0]+w[1]; assert abs(a[0]-w[0]/total)<1e-7 and abs(a[1]-w[1]/total)<1e-7 and a[2:]==(0,0)
report={'vertices':1880,'rawPackedWeightCount':wc,'rawPackedBoneCount':bc,'nativeWeightsMatchRaw':True,'nativePaletteIndicesMatchRaw':True,'repairedWeightsMatchTwoBonePath':True,'inputSha256':hashlib.sha256(source).hexdigest(),'outputSha256':hashlib.sha256(fixed).hexdigest()}
(root/'skin-audit.json').write_text(json.dumps(report,indent=2)); print(json.dumps(report))
