// Move the last runtime inventory snapshot into Editor documentation and emit
// native TableData references. Run from FusionForge with an explicit asset root.
import fs from 'node:fs';
import path from 'node:path';
import assert from 'node:assert/strict';
import crypto from 'node:crypto';

const root = path.resolve(process.argv[2]);
const archive = path.resolve('docs/legacy/ffone/retired-runtime-inventories-20260908');
fs.mkdirSync(archive, {recursive:true});
const audioBytes = fs.readFileSync(path.join(root, '_runtime/audio.json'));
const characterBytes = fs.readFileSync(path.join(root, '_runtime/characters.json'));
const audio = JSON.parse(audioBytes), characters = JSON.parse(characterBytes);
const tablePath = path.join(root, 'data/tables/table-set.json');
const tableText = fs.readFileSync(tablePath, 'utf8'), tables = JSON.parse(tableText);
assert(!tables.tables.some(t => t.name === 'native_asset_routes'));
const moves = [];
const rows = audio.assets.map(a => {
  const {files, ...row} = a;
  if (a.category !== 'voice') return {...row, path: files[0].path};
  const base = files.find(f => f.locale === 'en') ?? files[0];
  const relative = base.path.split('/').slice(3).join('/');
  for (const file of files) {
    const target = `audio/voice/${file.locale}/${relative}`;
    if (target === file.path) continue;
    const sourceAbs = path.resolve(root, file.path), targetAbs = path.resolve(root, target);
    for (const p of [sourceAbs,targetAbs]) assert(p.startsWith(root + path.sep));
    assert(!fs.existsSync(targetAbs), target);
    const bytes = fs.readFileSync(sourceAbs);
    assert.equal(bytes.subarray(0,4).toString(), 'OggS');
    moves.push({from:file.path, to:target, sha256:crypto.createHash('sha256').update(bytes).digest('hex')});
  }
  return {...row, path:relative};
});
const models = characters.models.map(({glbBlake3, ...model}) => model);
const table = {name:'native_asset_routes', value:{m_pAudioData:rows, m_pCharacterModelData:models}};
const end = tableText.lastIndexOf('\n  ]');
assert(end > 0);
const next = tableText.slice(0,end) + ',\n' + JSON.stringify(table,null,2).split('\n').map(l=>'    '+l).join('\n') + tableText.slice(end);
assert.equal(JSON.parse(next).tables.length,tables.tables.length+1);
for (const [name, bytes] of [['audio.json',audioBytes],['characters.json',characterBytes]]) {
  const p = path.join(archive,name);
  if(fs.existsSync(p)) assert(fs.readFileSync(p).equals(bytes)); else fs.writeFileSync(p,bytes);
}
fs.writeFileSync(path.join(archive,'relative-path-moves.json'),JSON.stringify(moves,null,2)+'\n');
for(const move of moves) fs.renameSync(path.resolve(root,move.from),path.resolve(root,move.to));
fs.writeFileSync(tablePath,next);
console.log({audioRows:rows.length,modelRows:models.length,renames:moves.length,archive});
// Remove the old runtime files only after the client and release readers have been migrated.
