#!/usr/bin/env python3
"""Generate the exact protocol-0104 wire mirror for FFOneClient.

Input: the decompiled ``Assembly-CSharp`` structs of the clean Retrobution
client (``sP_*.cs`` packets, ``s*.cs`` nested records and ``csDefines.cs``).
Every struct carries ``[StructLayout(Sequential, Pack = N, Size = M)]`` plus a
``MarshalAs`` attribute per field, so the marshaled ABI is fully determined.

Output:
  * ``crates/ffone-protocol/src/wire_0104.rs`` in FFOneClient;
  * ``crates/ffone-protocol/src/packet_ids_wire_0104.rs`` with every packet ID
    the hand-written ``packet`` module does not declare yet;
  * ``docs/reference/evidence/legacy/ffone/managed-code/wire-0104-openfusion-divergence.json``
    listing every struct whose clean-client layout differs from the pinned
    OpenFusion ``structs/0104.hpp`` (when that header is available).

Layout rules are the C ``#pragma pack`` rules Mono applies to sequential
structs.  Mono reports ``max(explicit Size, computed size)``; the generator
does the same and prints every struct where the two disagree so a stale
``Size`` attribute (a Retrobution field added without regenerating it) stays
visible.  An empty struct marshals as one byte, matching OpenFusion's
``uint8_t UNUSED`` placeholders.

Usage:
    python generate-wire-0104.py [--decompiled DIR] [--ffone DIR]
                                 [--openfusion-header FILE] [--check]
"""

from __future__ import annotations

import argparse
import json
import re
import sys
from dataclasses import dataclass, field
from pathlib import Path

HERE = Path(__file__).resolve().parent
EDITOR_ROOT = HERE.parents[1]
DEFAULT_DECOMPILED = (
    EDITOR_ROOT
    / "work/projects/retrobution-ui-20260821.ffclient/reports/avatar-animation-primary/decompiled"
)
DEFAULT_FFONE = EDITOR_ROOT.parent / "FFOneClient"
DEFAULT_OPENFUSION_HEADER = (
    EDITOR_ROOT
    / "work/ffone/legacy-content/imported/Kate/Kates FF Modding Workspace"
    / "rbution-server-SRC/OpenFusion-WIP-Retro/src/structs/0104.hpp"
)
DEFAULT_DIVERGENCE = (
    EDITOR_ROOT / "docs/reference/evidence/legacy/ffone/managed-code/wire-0104-openfusion-divergence.json"
)
DEFAULT_PACKETS_CPP = DEFAULT_OPENFUSION_HEADER.parents[1] / "core/Packets.cpp"

# Trailer records that only exist on the server side of the descriptor table.
OPENFUSION_TRAILER_SIZES = {
    "int32_t": 4,
    "int64_t": 8,
    # core/Packets.hpp: `struct sGM_PVPTarget { uint32_t eCT; uint32_t iID; }`.
    "sGM_PVPTarget": 8,
}

RUST_KEYWORDS = {
    "as", "break", "const", "continue", "crate", "else", "enum", "extern", "false", "fn",
    "for", "if", "impl", "in", "let", "loop", "match", "mod", "move", "mut", "pub", "ref",
    "return", "self", "static", "struct", "super", "trait", "true", "type", "unsafe", "use",
    "where", "while", "async", "await", "dyn", "abstract", "become", "box", "do", "final",
    "macro", "override", "priv", "typeof", "unsized", "virtual", "yield", "try",
}

PRIMITIVES = {
    # marshal: (size, natural align, rust)
    "I1": (1, 1, "i8"),
    "U1": (1, 1, "u8"),
    "I2": (2, 2, "i16"),
    "U2": (2, 2, "u16"),
    "I4": (4, 4, "i32"),
    "U4": (4, 4, "u32"),
    "I8": (8, 8, "i64"),
    "U8": (8, 8, "u64"),
    "R4": (4, 4, "f32"),
    "R8": (8, 8, "f64"),
}

CSHARP_ELEMENT = {
    "sbyte": "I1", "byte": "U1", "short": "I2", "ushort": "U2", "int": "I4", "uint": "U4",
    "long": "I8", "ulong": "U8", "float": "R4", "double": "R8",
}

CPP_PRIMITIVES = {
    "int8_t": 1, "uint8_t": 1, "char": 1, "int16_t": 2, "uint16_t": 2, "short": 2,
    "char16_t": 2, "int32_t": 4, "uint32_t": 4, "int": 4, "float": 4, "int64_t": 8,
    "uint64_t": 8, "double": 8,
}

ACRONYMS = {
    "PC": "Pc", "NPC": "Npc", "EP": "Ep", "GM": "Gm", "ID": "Id", "UID": "Uid", "HP": "Hp",
    "LS": "Ls", "FE": "Fe", "CL": "Cl", "IM": "Im", "MOTD": "Motd", "RXCOM": "Rxcom",
    "PCS": "Pcs", "NPCS": "Npcs", "CHARS": "Chars", "NPCTYPES": "NpcTypes",
}

# Nested records whose mechanical name would be unreadable.
RECORD_NAME_OVERRIDES = {
    "sPC_BATTERYs": "PcBatteries0104",
    "sPC_HP": "PcHp0104",
    "sPC_Nano": "PcNano0104",
    "sPC_NanoSlots": "PcNanoSlots0104",
    "sSYSTEMTIME": "SystemTime0104",
    "sCNStreetStall_ItemInfo_for_Client": "StreetStallItemInfo0104",
    "sEmailItemInfoFromCL": "EmailItemInfoFromClient0104",
    "sNanoTuneNeedItemInfo2CL": "NanoTuneNeedItemInfo0104",
    "sTimeLimitItemDeleteInfo2CL": "TimeLimitItemDeleteInfo0104",
    "sPCLoadData2CL": "PcLoadData0104",
    "sPCRegenDataForOtherPC": "PcRegenDataForOtherPc0104",
    "sSkillResult_Damage_N_Debuff": "SkillResultDamageAndDebuff0104",
    "sSkillResult_Damage_N_Move": "SkillResultDamageAndMove0104",
    "sSkillResult_Heal_HP": "SkillResultHealHp0104",
}


@dataclass
class Field:
    marshal: str
    ctype: str
    name: str
    size_const: int | None
    offset: int = 0
    size: int = 0
    align: int = 1
    rust_type: str = ""
    rust_name: str = ""
    kind: str = ""  # prim | utf16 | prim_array | struct | struct_array
    elem: str = ""


@dataclass
class Struct:
    name: str
    pack: int
    declared_size: int | None
    fields: list[Field] = field(default_factory=list)
    size: int = 0
    align: int = 1
    rust_name: str = ""
    is_packet: bool = False
    packet_id: int | None = None
    source_file: str = ""
    size_note: str = ""
    openfusion_size: int | None = None


LAYOUT_RE = re.compile(
    r"\[StructLayout\(LayoutKind\.Sequential(?:,\s*CharSet\s*=\s*CharSet\.\w+)?"
    r"(?:,\s*Pack\s*=\s*(\d+))?(?:,\s*Size\s*=\s*(\d+))?\)\]"
)
MARSHAL_RE = re.compile(
    r"\[MarshalAs\(UnmanagedType\.(\w+)(?:,\s*SizeConst\s*=\s*(\d+))?"
    r"(?:,\s*ArraySubType\s*=\s*UnmanagedType\.(\w+))?\)\]"
)
FIELD_RE = re.compile(r"public\s+([\w\[\]]+)\s+(\w+)\s*;")
STRUCT_RE = re.compile(r"public\s+struct\s+(\w+)")
DEFINE_RE = re.compile(r"public const uint (P_\w+) = (\d+)u;")


def parse_struct(path: Path) -> Struct | None:
    text = path.read_text(encoding="utf-8")
    layout = LAYOUT_RE.search(text)
    if not layout:
        return None
    struct_match = STRUCT_RE.search(text)
    if not struct_match:
        return None
    pack = int(layout.group(1)) if layout.group(1) else 8
    declared = int(layout.group(2)) if layout.group(2) else None
    struct = Struct(name=struct_match.group(1), pack=pack, declared_size=declared,
                    source_file=path.name)
    pending: tuple[str, int | None] | None = None
    for raw in text.splitlines():
        line = raw.strip()
        marshal = MARSHAL_RE.match(line)
        if marshal:
            pending = (marshal.group(1), int(marshal.group(2)) if marshal.group(2) else None)
            continue
        fld = FIELD_RE.match(line)
        if fld:
            if pending is None:
                raise SystemExit(f"{path.name}: field {fld.group(2)} has no MarshalAs")
            struct.fields.append(Field(pending[0], fld.group(1), fld.group(2), pending[1]))
            pending = None
    return struct


def pascal(token: str) -> str:
    if token in ACRONYMS:
        return ACRONYMS[token]
    if token.isdigit():
        return token
    return token[:1].upper() + token[1:].lower()


def packet_rust_name(csharp: str) -> str:
    assert csharp.startswith("sP_")
    tokens = csharp[3:].split("_")
    direction = tokens[0]
    rest = tokens[1:]
    is_request = "REQ" in rest
    is_reply = "REP" in rest
    rest = [t for t in rest if t not in ("REQ", "REP")]
    has_result = any(t in ("SUCC", "FAIL") for t in rest)
    words = []
    for t in rest:
        if t == "SUCC":
            words.append("Success")
        elif t == "FAIL":
            words.append("Failure")
        else:
            words.append(pascal(t))
    if not has_result:
        if is_request and direction.startswith("CL2"):
            words.append("Request")
        elif is_reply:
            words.append("Reply")
    if direction in ("CL2LS", "LS2CL"):
        # Login-server packets keep an explicit Ls prefix so they never shadow
        # shard packets with the same tail (e.g. PC_EXIT_DUPLICATE).
        words.insert(0, "Ls")
    return "".join(words) + "0104"


def nested_rust_name(csharp: str) -> str:
    if csharp in RECORD_NAME_OVERRIDES:
        return RECORD_NAME_OVERRIDES[csharp]
    assert csharp.startswith("s")
    out = []
    for part in csharp[1:].split("_"):
        if not part:
            continue
        if part.isupper():
            out.append(pascal(part))
        else:
            chunks = re.findall(r"[A-Z]+(?=[A-Z][a-z])|[A-Z]?[a-z0-9]+|[A-Z]+", part)
            out.extend(pascal(c) for c in chunks)
    return "".join(out) + "0104"


def snake(name: str) -> str:
    m = re.match(r"^(sz|ui|dw|i|a|e|f|b|u|w|k|p)([A-Z].*)$", name)
    if m:
        prefix, rest = m.group(1), m.group(2)
        if prefix == "e" and len(rest) <= 3 and rest.isupper():
            name = "e_" + rest.lower()
        else:
            name = rest
    s = re.sub(r"(?<=[a-z0-9])(?=[A-Z])", "_", name)
    s = re.sub(r"(?<=[A-Z])(?=[A-Z][a-z])", "_", s)
    s = re.sub(r"_+", "_", s.lower()).strip("_")
    if s in RUST_KEYWORDS:
        s += "_"
    if not s or s[0].isdigit():
        s = "f_" + s
    return s


def align_up(value: int, align: int) -> int:
    return (value + align - 1) // align * align


def layout(struct: Struct, table: dict[str, Struct], resolving: set[str], notes: list[str]) -> None:
    if struct.size:
        return
    if struct.name in resolving:
        raise SystemExit(f"recursive struct {struct.name}")
    resolving.add(struct.name)
    offset = 0
    max_align = 1
    seen: set[str] = set()
    for f in struct.fields:
        if f.marshal in PRIMITIVES:
            size, natural, rust = PRIMITIVES[f.marshal]
            f.kind, f.size, f.rust_type, f.elem = "prim", size, rust, f.marshal
            f.align = min(natural, struct.pack)
        elif f.marshal == "ByValTStr":
            assert f.size_const is not None
            f.kind, f.size = "utf16", f.size_const * 2
            f.rust_type = f"FixedUtf16<{f.size_const}>"
            f.align = min(2, struct.pack)
        elif f.marshal == "ByValArray":
            assert f.size_const is not None
            elem_ctype = f.ctype.rstrip("[]")
            if elem_ctype in CSHARP_ELEMENT:
                marshal = CSHARP_ELEMENT[elem_ctype]
                size, natural, rust = PRIMITIVES[marshal]
                f.kind, f.elem = "prim_array", marshal
                f.size = size * f.size_const
                f.align = min(natural, struct.pack)
                f.rust_type = f"[{rust}; {f.size_const}]"
            else:
                nested = table.get(elem_ctype)
                if nested is None:
                    raise SystemExit(f"{struct.name}.{f.name}: unknown nested {elem_ctype}")
                layout(nested, table, resolving, notes)
                f.kind, f.elem = "struct_array", nested.name
                f.size = nested.size * f.size_const
                f.align = min(nested.align, struct.pack)
                f.rust_type = f"[{nested.rust_name}; {f.size_const}]"
        elif f.marshal == "Struct":
            nested = table.get(f.ctype)
            if nested is None:
                raise SystemExit(f"{struct.name}.{f.name}: unknown nested {f.ctype}")
            layout(nested, table, resolving, notes)
            f.kind, f.elem = "struct", nested.name
            f.size = nested.size
            f.align = min(nested.align, struct.pack)
            f.rust_type = nested.rust_name
        else:
            raise SystemExit(f"{struct.name}.{f.name}: unsupported marshal {f.marshal}")
        offset = align_up(offset, f.align)
        f.offset = offset
        offset += f.size
        max_align = max(max_align, f.align)
        f.rust_name = snake(f.name)
        if f.rust_name in seen:
            f.rust_name = snake(f.name + "_" + f.marshal.lower())
        seen.add(f.rust_name)
    struct.align = max_align
    computed = align_up(offset, max_align) if offset else 0
    if not struct.fields:
        # Mono marshals an empty sequential struct as one byte; OpenFusion
        # declares the same packets with a `uint8_t UNUSED` placeholder.
        computed = 1
        struct.size_note = "empty struct: one placeholder byte"
    if struct.declared_size is not None and struct.declared_size != computed:
        struct.size = max(struct.declared_size, computed)
        struct.size_note = (
            f"source declares Size = {struct.declared_size} but the fields need {computed} bytes; "
            f"Mono marshals max(explicit, computed) = {struct.size}"
        )
        notes.append(f"{struct.name}: {struct.size_note}")
    else:
        struct.size = computed
    if struct.declared_size is None:
        struct.declared_size = struct.size
    resolving.discard(struct.name)


def parse_openfusion_header(path: Path) -> dict[str, int]:
    """Return struct sizes computed from OpenFusion's generated 0104 header."""
    sizes: dict[str, int] = {}
    aligns: dict[str, int] = {}
    pack = 8
    current: str | None = None
    offset = 0
    max_align = 1
    for raw in path.read_text(encoding="utf-8").splitlines():
        line = raw.strip()
        pragma = re.match(r"#pragma pack\((\d+)\)", line)
        if pragma:
            pack = int(pragma.group(1))
            continue
        start = re.match(r"struct (\w+) \{", line)
        if start:
            current, offset, max_align = start.group(1), 0, 1
            continue
        if line == "};" and current:
            size = align_up(offset, max_align) if offset else 1
            sizes[current] = size
            aligns[current] = max_align
            current = None
            continue
        if current is None:
            continue
        fld = re.match(r"(\w+) (\w+)((?:\[\d+\])*);", line)
        if not fld:
            continue
        ctype = fld.group(1)
        count = 1
        for dim in re.findall(r"\[(\d+)\]", fld.group(3)):
            count *= int(dim)
        if ctype in CPP_PRIMITIVES:
            elem_size = CPP_PRIMITIVES[ctype]
            natural = elem_size
        elif ctype in sizes:
            elem_size = sizes[ctype]
            natural = aligns[ctype]
        else:
            raise SystemExit(f"OpenFusion header: unknown type {ctype} in {current}")
        align = min(natural, pack)
        offset = align_up(offset, align) + elem_size * count
        max_align = max(max_align, align)
    return sizes


DESCRIPTOR_RE = re.compile(r"^\s*(PACKET|VAR_PACKET|MANUAL)\((P_\w+)(?:,\s*(\w+),\s*(\w+))?\)")


def parse_packets_cpp(path: Path) -> dict[str, tuple[str, str | None, str | None]]:
    """Return `name -> (kind, count_member, trailer_type)` from OpenFusion's table."""
    table: dict[str, tuple[str, str | None, str | None]] = {}
    for raw in path.read_text(encoding="utf-8").splitlines():
        m = DESCRIPTOR_RE.match(raw)
        if m:
            table[m.group(2)] = (m.group(1), m.group(3), m.group(4))
    return table


def emit_struct(struct: Struct) -> str:
    out = []
    kind = "packet" if struct.is_packet else "record"
    id_note = f", packet ID `0x{struct.packet_id:08x}`" if struct.packet_id is not None else ""
    out.append(f"/// `{struct.name}` (`#pragma pack({struct.pack})`, {struct.size} bytes{id_note}).")
    out.append("///")
    out.append(f"/// Generated {kind} mirror of the clean Retrobution marshaled layout.")
    if struct.size_note:
        out.append("///")
        out.append(f"/// Layout note: {struct.size_note}.")
    if struct.openfusion_size is not None and struct.openfusion_size != struct.size:
        out.append("///")
        out.append(
            f"/// OpenFusion divergence: the pinned `structs/0104.hpp` declares this struct as "
            f"{struct.openfusion_size} bytes. Wiring code must choose the server-compatible "
            "length explicitly."
        )
    out.append("#[derive(Debug, Clone, PartialEq)]")
    if struct.fields:
        out.append(f"pub struct {struct.rust_name} {{")
        for f in struct.fields:
            out.append(f"    /// `{f.name}` at offset {f.offset}.")
            out.append(f"    pub {f.rust_name}: {f.rust_type},")
        out.append("}")
    else:
        out.append(f"pub struct {struct.rust_name};")
    out.append("")
    out.append(f"impl {struct.rust_name} {{")
    out.append(f"    pub const SIZE: usize = {struct.size};")
    out.append("")
    out.append("    /// Write the exact layout into `out`, which must be `SIZE` bytes.")
    out.append("    pub fn write_into(&self, out: &mut [u8]) {")
    out.append("        debug_assert_eq!(out.len(), Self::SIZE);")
    if not struct.fields:
        out.append("        out[0] = 0;")
    for f in struct.fields:
        o = f.offset
        if f.kind == "prim":
            out.append(f"        write_prim(out, {o}, &self.{f.rust_name}.to_le_bytes());")
        elif f.kind == "utf16":
            out.append(f"        write_utf16(out, {o}, &self.{f.rust_name});")
        elif f.kind == "prim_array":
            esize = PRIMITIVES[f.elem][0]
            out.append(f"        for (index, value) in self.{f.rust_name}.iter().enumerate() {{")
            out.append(f"            write_prim(out, {o} + index * {esize}, &value.to_le_bytes());")
            out.append("        }")
        elif f.kind == "struct":
            out.append(f"        self.{f.rust_name}.write_into(&mut out[{o}..{o + f.size}]);")
        elif f.kind == "struct_array":
            esize = f.size // f.size_const
            out.append(f"        for (index, value) in self.{f.rust_name}.iter().enumerate() {{")
            out.append(f"            let start = {o} + index * {esize};")
            out.append(f"            value.write_into(&mut out[start..start + {esize}]);")
            out.append("        }")
    out.append("    }")
    out.append("")
    out.append("    /// Read the exact layout from `bytes`, which must be `SIZE` bytes.")
    out.append("    pub fn read_from(bytes: &[u8]) -> Self {")
    out.append("        debug_assert_eq!(bytes.len(), Self::SIZE);")
    if not struct.fields:
        out.append("        Self")
    else:
        out.append("        Self {")
        for f in struct.fields:
            o = f.offset
            if f.kind == "prim":
                out.append(
                    f"            {f.rust_name}: {f.rust_type}::from_le_bytes(read_array(bytes, {o})),"
                )
            elif f.kind == "utf16":
                out.append(f"            {f.rust_name}: read_utf16(bytes, {o}),")
            elif f.kind == "prim_array":
                esize, _, rust = PRIMITIVES[f.elem]
                out.append(f"            {f.rust_name}: std::array::from_fn(|index| {{")
                out.append(f"                {rust}::from_le_bytes(read_array(bytes, {o} + index * {esize}))")
                out.append("            }),")
            elif f.kind == "struct":
                out.append(
                    f"            {f.rust_name}: {f.rust_type}::read_from(&bytes[{o}..{o + f.size}]),"
                )
            elif f.kind == "struct_array":
                esize = f.size // f.size_const
                elem_rust = f.rust_type[1:].split(";")[0]
                out.append(f"            {f.rust_name}: std::array::from_fn(|index| {{")
                out.append(f"                let start = {o} + index * {esize};")
                out.append(f"                {elem_rust}::read_from(&bytes[start..start + {esize}])")
                out.append("            }),")
        out.append("        }")
    out.append("    }")
    out.append("}")
    out.append("")
    out.append(f"impl WirePayload for {struct.rust_name} {{")
    out.append("    const SIZE: usize = Self::SIZE;")
    out.append("")
    out.append("    fn encode(&self) -> Vec<u8> {")
    out.append("        let mut out = vec![0; Self::SIZE];")
    out.append("        self.write_into(&mut out);")
    out.append("        out")
    out.append("    }")
    out.append("")
    out.append("    fn decode(bytes: &[u8]) -> Result<Self, PayloadError> {")
    out.append("        require_size(bytes, Self::SIZE)?;")
    out.append("        Ok(Self::read_from(bytes))")
    out.append("    }")
    out.append("}")
    out.append("")
    return "\n".join(out)


HEADER = '''//! Exact marshaled mirror of every `[StructLayout]` struct in the clean
//! Retrobution `Assembly-CSharp` (protocol 0104).
//!
//! GENERATED FILE. Do not edit by hand. Regenerate with
//! `../FusionForge/tools/legacy-sources/generate-wire-0104.py`.
//!
//! Every type below reproduces the original Mono marshaled layout byte for
//! byte: the generator recomputes each offset with the `#pragma pack` rules,
//! compares the result with the decompiled `Size = N` attribute and documents
//! the single struct where Retrobution left a stale attribute. Names are
//! derived mechanically from the C# identifiers; the original field name is
//! kept in each field's doc comment.
//!
//! These are *wire* types. Curated semantic types with validation live at the
//! crate root; where both exist, `tests::hand_written_sizes_agree_with_mirror`
//! proves the two agree on the fixed payload size.
//!
//! [`declared_struct_size`] reports the marshaled struct size of a packet ID.
//! Packets that carry a counted trailer (`iNPCCnt`, `iTargetCnt`, ...) are
//! longer on the wire than their declared struct; [`frame_kind`] tells the
//! three shapes apart using OpenFusion's own descriptor table
//! (`core/Packets.cpp`), and [`fixed_frame_size`] answers only for packets the
//! server validates as fixed-length. `fixed_payload_size` at the crate root
//! remains the authority for strict length classification; it falls back to
//! [`fixed_frame_size`] for packets without a hand-written codec.

#![allow(clippy::too_many_lines)]

use crate::{FixedUtf16, PayloadError, WirePayload};

fn require_size(bytes: &[u8], expected: usize) -> Result<(), PayloadError> {
    if bytes.len() == expected {
        Ok(())
    } else {
        Err(PayloadError::WrongSize {
            expected,
            actual: bytes.len(),
        })
    }
}

#[inline]
fn write_prim(out: &mut [u8], offset: usize, value: &[u8]) {
    out[offset..offset + value.len()].copy_from_slice(value);
}

#[inline]
fn read_array<const N: usize>(bytes: &[u8], offset: usize) -> [u8; N] {
    bytes[offset..offset + N].try_into().expect("fixed slice")
}

fn write_utf16<const N: usize>(out: &mut [u8], offset: usize, value: &FixedUtf16<N>) {
    for (index, unit) in value.as_units().iter().enumerate() {
        let start = offset + index * 2;
        out[start..start + 2].copy_from_slice(&unit.to_le_bytes());
    }
}

fn read_utf16<const N: usize>(bytes: &[u8], offset: usize) -> FixedUtf16<N> {
    FixedUtf16::from_units(std::array::from_fn(|index| {
        let start = offset + index * 2;
        u16::from_le_bytes([bytes[start], bytes[start + 1]])
    }))
}

'''


def rustfmt(source: str, ffone: Path) -> str:
    """Format generated Rust with the FFOne toolchain's rustfmt so the committed
    file is byte-identical to what `cargo fmt` would produce."""
    import subprocess

    result = subprocess.run(
        ["rustfmt", "--edition", "2024", "--config-path", str(ffone / "rustfmt.toml")],
        input=source,
        capture_output=True,
        text=True,
        encoding="utf-8",
        cwd=ffone,
        check=False,
    )
    if result.returncode != 0:
        raise SystemExit(f"rustfmt failed:\n{result.stderr}")
    return result.stdout


from partition_wire import partition_wire

def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--decompiled", type=Path, default=DEFAULT_DECOMPILED)
    parser.add_argument("--ffone", type=Path, default=DEFAULT_FFONE)
    parser.add_argument("--openfusion-header", type=Path, default=DEFAULT_OPENFUSION_HEADER)
    parser.add_argument("--divergence", type=Path, default=DEFAULT_DIVERGENCE)
    parser.add_argument("--packets-cpp", type=Path, default=DEFAULT_PACKETS_CPP)
    parser.add_argument("--check", action="store_true", help="fail if output would change")
    args = parser.parse_args()

    decompiled: Path = args.decompiled
    ffone: Path = args.ffone
    lib_rs = ffone / "crates/ffone-protocol/src/lib.rs"
    out_rs = ffone / "crates/ffone-protocol/src/wire_0104.rs"
    ids_rs = ffone / "crates/ffone-protocol/src/packet_ids_wire_0104.rs"

    defines = {}
    for m in DEFINE_RE.finditer((decompiled / "csDefines.cs").read_text(encoding="utf-8")):
        defines[m.group(1)] = int(m.group(2))

    table: dict[str, Struct] = {}
    for path in sorted(decompiled.glob("s*.cs")):
        if path.name.startswith("sPacket"):
            continue
        parsed = parse_struct(path)
        if parsed is not None:
            table[parsed.name] = parsed

    for struct in table.values():
        if struct.name.startswith("sP_"):
            struct.is_packet = True
            define = struct.name[1:]
            if define not in defines:
                raise SystemExit(f"{struct.name}: no csDefines ID")
            struct.packet_id = defines[define]
            struct.rust_name = packet_rust_name(struct.name)
        else:
            struct.rust_name = nested_rust_name(struct.name)

    packet_names = {s.rust_name for s in table.values() if s.is_packet}
    for struct in table.values():
        if not struct.is_packet and struct.rust_name in packet_names:
            renamed = struct.rust_name[: -len("0104")] + "Record0104"
            print(f"record {struct.name}: {struct.rust_name} -> {renamed} (packet collision)")
            struct.rust_name = renamed
    names: dict[str, str] = {}
    for struct in table.values():
        if struct.rust_name in names:
            raise SystemExit(
                f"name collision {struct.rust_name}: {names[struct.rust_name]} vs {struct.name}"
            )
        names[struct.rust_name] = struct.name

    notes: list[str] = []
    resolving: set[str] = set()
    for struct in table.values():
        layout(struct, table, resolving, notes)
    for note in notes:
        print("layout note:", note)

    divergence = {
        "schema": "ffone.wire-0104-openfusion-divergence.v1",
        "clean_client": "retrobution-20260821 Assembly-CSharp (decompiled)",
        "openfusion_header": None,
        "size_mismatch": [],
        "retrobution_only": [],
        "openfusion_only": [],
    }
    header: Path = args.openfusion_header
    if header.exists():
        divergence["openfusion_header"] = str(header.relative_to(EDITOR_ROOT)).replace("\\", "/")
        of_sizes = parse_openfusion_header(header)
        for struct in table.values():
            of_size = of_sizes.get(struct.name)
            struct.openfusion_size = of_size
            if of_size is None:
                divergence["retrobution_only"].append(struct.name)
            elif of_size != struct.size:
                divergence["size_mismatch"].append(
                    {"struct": struct.name, "retrobution": struct.size, "openfusion": of_size}
                )
        for name in of_sizes:
            if name not in table:
                divergence["openfusion_only"].append(name)
        for key in ("retrobution_only", "openfusion_only"):
            divergence[key].sort()
        print(
            f"openfusion cross-check: {len(divergence['size_mismatch'])} size mismatches, "
            f"{len(divergence['retrobution_only'])} retrobution-only, "
            f"{len(divergence['openfusion_only'])} openfusion-only"
        )
        for entry in divergence["size_mismatch"]:
            print("  mismatch:", entry)
    else:
        of_sizes = {}
        print(f"openfusion header not found, skipping cross-check: {header}")

    # Frame shapes: (kind, header_size, count_offset, trailer_size) per packet.
    descriptors = parse_packets_cpp(args.packets_cpp) if args.packets_cpp.exists() else {}
    if not descriptors:
        print(f"packet descriptor table not found, frame kinds will be empty: {args.packets_cpp}")
    frame_kinds: list[tuple[Struct, str, int | None, int | None]] = []
    for s in table.values():
        if not s.is_packet:
            continue
        desc = descriptors.get(s.name[1:])
        if desc is None:
            continue
        kind, member, trailer = desc
        if kind == "PACKET":
            frame_kinds.append((s, "Fixed", None, None))
        elif kind == "MANUAL":
            frame_kinds.append((s, "Manual", None, None))
        else:
            count_field = next((f for f in s.fields if f.name == member), None)
            if count_field is None:
                raise SystemExit(f"{s.name}: count member {member} is not a clean-client field")
            if trailer in OPENFUSION_TRAILER_SIZES:
                trailer_size = OPENFUSION_TRAILER_SIZES[trailer]
            elif trailer in table:
                trailer_size = table[trailer].size
            elif trailer in of_sizes:
                trailer_size = of_sizes[trailer]
            else:
                raise SystemExit(f"{s.name}: unknown trailer type {trailer}")
            frame_kinds.append((s, "Counted", (count_field.offset, count_field.size), trailer_size))
    kind_counts = {k: sum(1 for _, kind, _, _ in frame_kinds if kind == k)
                   for k in ("Fixed", "Counted", "Manual")}
    print(f"frame kinds: {kind_counts}, {len(packets_without := [s for s in table.values() if s.is_packet and s.name[1:] not in descriptors])} packets without a descriptor")

    existing_ids = set(re.findall(r"pub const (P_\w+): u32", lib_rs.read_text(encoding="utf-8")))

    ordered = sorted(table.values(), key=lambda s: (s.is_packet, s.packet_id or 0, s.name))
    nested = [s for s in ordered if not s.is_packet]
    packets = [s for s in ordered if s.is_packet]

    body = [HEADER]
    body.append("// ---- nested records -------------------------------------------------------\n")
    body.extend(emit_struct(s) for s in nested)
    body.append("// ---- packets ----------------------------------------------------------------\n")
    body.extend(emit_struct(s) for s in packets)

    body.append("/// Declared marshaled struct size for every clean-client packet ID.")
    body.append("///")
    body.append("/// Counted-trailer packets are longer on the wire; see the module docs.")
    body.append("#[must_use]")
    body.append("pub fn declared_struct_size(packet_id: u32) -> Option<usize> {")
    body.append("    Some(match packet_id {")
    for s in packets:
        body.append(f"        0x{s.packet_id:08x} => {s.rust_name}::SIZE,")
    body.append("        _ => return None,")
    body.append("    })")
    body.append("}")
    body.append("")
    body.append("/// Every packet ID of the clean client with its `csDefines` name.")
    body.append("pub const PACKET_IDS_0104: &[(&str, u32)] = &[")
    for s in packets:
        body.append(f'    ("{s.name[1:]}", 0x{s.packet_id:08x}),')
    body.append("];")
    body.append("")
    body.append("/// Shape of one packet on the wire.")
    body.append("///")
    body.append("/// Kinds come from OpenFusion's descriptor table (`core/Packets.cpp`);")
    body.append("/// header sizes and count offsets come from the clean-client layout.")
    body.append("#[derive(Debug, Clone, Copy, PartialEq, Eq)]")
    body.append("pub enum FrameKind0104 {")
    body.append("    /// The payload is exactly the declared struct.")
    body.append("    Fixed { size: usize },")
    body.append("    /// The declared struct is followed by `count` trailer records, where")
    body.append("    /// `count` is the little-endian signed integer of `count_width` bytes")
    body.append("    /// stored at `count_offset` inside the header.")
    body.append("    Counted {")
    body.append("        header_size: usize,")
    body.append("        count_offset: usize,")
    body.append("        count_width: usize,")
    body.append("        trailer_size: usize,")
    body.append("    },")
    body.append("    /// Variadic with a trailer whose type depends on the body; only a")
    body.append("    /// dedicated decoder can validate it.")
    body.append("    Manual { header_size: usize },")
    body.append("}")
    body.append("")
    body.append("/// Wire shape of a packet, when OpenFusion's descriptor table knows it.")
    body.append("///")
    body.append("/// Retrobution-only packets absent from that table return `None`.")
    body.append("#[must_use]")
    body.append("pub fn frame_kind(packet_id: u32) -> Option<FrameKind0104> {")
    body.append("    Some(match packet_id {")
    for s, kind, count, trailer_size in frame_kinds:
        if kind == "Fixed":
            body.append(f"        0x{s.packet_id:08x} => FrameKind0104::Fixed {{ size: {s.rust_name}::SIZE }},")
        elif kind == "Manual":
            body.append(
                f"        0x{s.packet_id:08x} => FrameKind0104::Manual {{ header_size: {s.rust_name}::SIZE }},"
            )
        else:
            count_offset, count_width = count
            body.append(f"        0x{s.packet_id:08x} => FrameKind0104::Counted {{")
            body.append(f"            header_size: {s.rust_name}::SIZE,")
            body.append(f"            count_offset: {count_offset},")
            body.append(f"            count_width: {count_width},")
            body.append(f"            trailer_size: {trailer_size},")
            body.append("        },")
    body.append("        _ => return None,")
    body.append("    })")
    body.append("}")
    body.append("")
    body.append("/// Exact payload length for packets the server validates as fixed-length.")
    body.append("#[must_use]")
    body.append("pub fn fixed_frame_size(packet_id: u32) -> Option<usize> {")
    body.append("    match frame_kind(packet_id)? {")
    body.append("        FrameKind0104::Fixed { size } => Some(size),")
    body.append("        FrameKind0104::Counted { .. } | FrameKind0104::Manual { .. } => None,")
    body.append("    }")
    body.append("}")
    body.append("")
    body.append("/// Packets whose clean-client size differs from the pinned OpenFusion")
    body.append("/// `structs/0104.hpp`: `(name, id, retrobution_size, openfusion_size)`.")
    body.append("///")
    body.append("/// Hand-written codecs may deliberately follow the server side for these;")
    body.append("/// the mirror always follows the client. Source:")
    body.append("/// `../FusionForge/docs/reference/evidence/legacy/ffone/managed-code/wire-0104-openfusion-divergence.json`.")
    body.append("pub const OPENFUSION_SIZE_DIVERGENCES_0104: &[(&str, u32, usize, usize)] = &[")
    for s in packets:
        if s.openfusion_size is not None and s.openfusion_size != s.size:
            body.append(
                f'    ("{s.name[1:]}", 0x{s.packet_id:08x}, {s.size}, {s.openfusion_size}),'
            )
    body.append("];")
    body.append("")

    body.append("#[cfg(test)]")
    body.append("mod tests {")
    body.append("    use super::*;")
    body.append("    use crate::{fixed_payload_size, packet};")
    body.append("")
    body.append("    fn pattern(len: usize) -> Vec<u8> {")
    body.append("        (0..len).map(|index| (index * 7 + 3) as u8).collect()")
    body.append("    }")
    body.append("")
    body.append("    fn round_trip<T: WirePayload + PartialEq + std::fmt::Debug>(declared: usize) {")
    body.append("        assert_eq!(T::SIZE, declared);")
    body.append("        let first = T::decode(&pattern(declared)).expect(\"declared size decodes\");")
    body.append("        let wire = first.encode();")
    body.append("        assert_eq!(wire.len(), declared);")
    body.append("        assert_eq!(T::decode(&wire).unwrap(), first);")
    body.append("        assert!(T::decode(&pattern(declared + 1)).is_err());")
    body.append("        assert!(T::decode(&pattern(declared - 1)).is_err());")
    body.append("    }")
    body.append("")
    body.append("    #[test]")
    body.append("    fn every_generated_layout_matches_its_declared_size_and_round_trips() {")
    for s in ordered:
        body.append(f"        round_trip::<{s.rust_name}>({s.size});")
    body.append("    }")
    body.append("")
    body.append("    #[test]")
    body.append("    fn every_packet_id_matches_the_clean_csdefines_value() {")
    for s in packets:
        # `csDefines` spells counted families with a trailing lowercase `s`
        # (`P_FE2CL_PC_ATTACK_NPCs`); the Rust constants are upper-case.
        body.append(f"        assert_eq!(packet::{s.name[1:].upper()}, 0x{s.packet_id:08x});")
    body.append(f"        assert_eq!(PACKET_IDS_0104.len(), {len(packets)});")
    body.append("    }")
    body.append("")
    body.append("    #[test]")
    body.append("    fn frame_kinds_follow_the_openfusion_descriptor_table() {")
    body.append("        let mut fixed = 0;")
    body.append("        let mut counted = 0;")
    body.append("        let mut manual = 0;")
    body.append("        for (_, id) in PACKET_IDS_0104 {")
    body.append("            match frame_kind(*id) {")
    body.append("                Some(FrameKind0104::Fixed { size }) => {")
    body.append("                    fixed += 1;")
    body.append("                    assert_eq!(fixed_frame_size(*id), Some(size));")
    body.append("                    assert_eq!(declared_struct_size(*id), Some(size));")
    body.append("                }")
    body.append("                Some(FrameKind0104::Counted {")
    body.append("                    header_size,")
    body.append("                    count_offset,")
    body.append("                    count_width,")
    body.append("                    trailer_size,")
    body.append("                }) => {")
    body.append("                    counted += 1;")
    body.append("                    assert!(matches!(count_width, 1 | 2 | 4));")
    body.append("                    assert!(count_offset + count_width <= header_size);")
    body.append("                    assert!(trailer_size > 0);")
    body.append("                    assert_eq!(fixed_frame_size(*id), None);")
    body.append("                }")
    body.append("                Some(FrameKind0104::Manual { header_size }) => {")
    body.append("                    manual += 1;")
    body.append("                    assert_eq!(declared_struct_size(*id), Some(header_size));")
    body.append("                    assert_eq!(fixed_frame_size(*id), None);")
    body.append("                }")
    body.append("                None => {}")
    body.append("            }")
    body.append("        }")
    body.append(f"        assert_eq!((fixed, counted, manual), ({kind_counts['Fixed']}, {kind_counts['Counted']}, {kind_counts['Manual']}));")
    body.append("    }")
    body.append("")
    body.append("    #[test]")
    body.append("    fn hand_written_sizes_agree_with_mirror() {")
    body.append("        for (name, id) in PACKET_IDS_0104 {")
    body.append("            let Some(fixed) = fixed_payload_size(*id) else {")
    body.append("                continue;")
    body.append("            };")
    body.append("            let declared = declared_struct_size(*id).unwrap();")
    body.append("            if fixed == declared {")
    body.append("                continue;")
    body.append("            }")
    body.append("            // A hand-written codec may follow the pinned server only where the")
    body.append("            // clean client and OpenFusion are known to disagree.")
    body.append("            let openfusion = OPENFUSION_SIZE_DIVERGENCES_0104")
    body.append("                .iter()")
    body.append("                .find(|(_, divergent_id, _, _)| divergent_id == id)")
    body.append("                .map(|(_, _, _, openfusion)| *openfusion);")
    body.append("            assert_eq!(")
    body.append("                Some(fixed),")
    body.append("                openfusion,")
    body.append("                \"{name}: fixed size {fixed} matches neither the clean client ({declared}) nor a documented OpenFusion divergence\"")
    body.append("            );")
    body.append("        }")
    body.append("    }")
    body.append("}")
    body.append("")
    wire_text = "\n".join(body)

    missing = [s for s in packets if s.name[1:].upper() not in existing_ids]
    ids_lines = [
        "// GENERATED by ../FusionForge/tools/legacy-sources/generate-wire-0104.py.",
        "// Packet IDs of the clean Retrobution client that the hand-written list above",
        "// does not declare. Included inside `pub mod packet`. Names are upper-cased",
        "// (`csDefines` writes counted families as `..._NPCs`).",
        "",
    ]
    ids_lines.extend(
        f"pub const {s.name[1:].upper()}: u32 = 0x{s.packet_id:08x};" for s in missing
    )
    ids_text = "\n".join(ids_lines) + "\n"
    divergence_text = json.dumps(divergence, indent=2) + "\n"

    outputs = [
        *((out_rs.parent / relative, text)
          for relative, text in partition_wire(rustfmt(wire_text, ffone)).items()),
        (ids_rs, rustfmt(ids_text, ffone)),
        (args.divergence, divergence_text),
    ]
    expected_wire_paths = {path for path, _ in outputs if path.suffix == ".rs"}
    generated_wire_dir = out_rs.with_suffix("")
    stale_wire_paths = sorted(
        path for path in generated_wire_dir.rglob("*.rs")
        if path not in expected_wire_paths
    )
    if args.check:
        changed = [str(p) for p, text in outputs
                   if not p.exists() or p.read_text(encoding="utf-8") != text]
        changed.extend(f"stale generated file: {p}" for p in stale_wire_paths)
        if changed:
            print("out of date:", *changed, sep="\n  ")
            return 1
        print("up to date")
        return 0

    # This directory is generator-owned; unknown files require an explicit decision.
    unknown = [p for p in stale_wire_paths if not p.read_text(encoding="utf-8").startswith(
        ("// Generated", "// GENERATED", "use super::*;"))]
    if unknown:
        print("unrecognized files in generated wire directory:", *unknown, sep="\n  ")
        return 1
    for path, text in outputs:
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text(text, encoding="utf-8", newline="\n")
    for path in stale_wire_paths:
        path.unlink()
    print(f"structs: {len(table)} ({len(packets)} packets, {len(nested)} records)")
    print(f"new packet IDs: {len(missing)}")
    for path, _ in outputs:
        print(f"wrote {path}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
