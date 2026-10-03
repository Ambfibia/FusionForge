import fs from "node:fs";
import path from "node:path";

const [, , sourcePath, outputPath, semanticName, sourceBlake3] = process.argv;
if (!sourcePath || !outputPath || !semanticName || !sourceBlake3) {
  throw new Error(
    "usage: node tools/legacy-sources/ffone-migration/promote_native_world_glb.mjs <source.glb> <output.glb> <semantic-name> <source-blake3>",
  );
}
if (!/^[0-9a-f]{64}$/.test(sourceBlake3)) {
  throw new Error("source BLAKE3 must be 64 lowercase hexadecimal characters");
}

const source = fs.readFileSync(sourcePath);
if (
  source.length < 28 ||
  source.toString("ascii", 0, 4) !== "glTF" ||
  source.readUInt32LE(4) !== 2 ||
  source.readUInt32LE(8) !== source.length
) {
  throw new Error("source is not a complete GLB 2.0 file");
}
const jsonLength = source.readUInt32LE(12);
if (
  source.readUInt32LE(16) !== 0x4e4f534a ||
  20 + jsonLength > source.length
) {
  throw new Error("source GLB has no valid leading JSON chunk");
}

const document = JSON.parse(source.subarray(20, 20 + jsonLength).toString("utf8").trimEnd());
if (
  !Array.isArray(document.nodes) ||
  document.nodes.length !== 1 ||
  !Array.isArray(document.meshes) ||
  document.meshes.length !== 1 ||
  document.nodes[0].mesh !== 0
) {
  throw new Error("world terrain promotion requires one root node and one mesh");
}

document.nodes[0].name = semanticName;
document.meshes[0].name = semanticName;
document.extras = {
  ...(document.extras ?? {}),
  ffoneNativeWorld: {
    semanticName,
    sourceGlb: sourcePath.replaceAll("\\", "/"),
    sourceGlbBlake3: sourceBlake3,
  },
};

let json = Buffer.from(JSON.stringify(document), "utf8");
const padding = (4 - (json.length % 4)) % 4;
if (padding !== 0) {
  json = Buffer.concat([json, Buffer.alloc(padding, 0x20)]);
}
const remainingChunks = source.subarray(20 + jsonLength);
const totalLength = 20 + json.length + remainingChunks.length;
if (totalLength > 0xffff_ffff) {
  throw new Error("promoted GLB exceeds the GLB u32 length limit");
}

const output = Buffer.alloc(totalLength);
output.write("glTF", 0, "ascii");
output.writeUInt32LE(2, 4);
output.writeUInt32LE(totalLength, 8);
output.writeUInt32LE(json.length, 12);
output.writeUInt32LE(0x4e4f534a, 16);
json.copy(output, 20);
remainingChunks.copy(output, 20 + json.length);

fs.mkdirSync(path.dirname(outputPath), { recursive: true });
const temporary = `${outputPath}.tmp`;
fs.writeFileSync(temporary, output);
if (fs.existsSync(outputPath)) {
  const current = fs.readFileSync(outputPath);
  if (!current.equals(output)) {
    fs.rmSync(temporary);
    throw new Error(`refusing to overwrite different promoted GLB: ${outputPath}`);
  }
  fs.rmSync(temporary);
} else {
  fs.renameSync(temporary, outputPath);
}

console.log(
  JSON.stringify({
    source: sourcePath,
    output: outputPath,
    semanticName,
    bytes: output.length,
  }),
);
