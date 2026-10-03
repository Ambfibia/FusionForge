"""Deterministic, lossless item-level partitioning of the generated wire mirror."""
from collections import defaultdict
from pathlib import Path
import re
from rust_items import parse

MAX_MODULE_LINES = 900


def _snake(name: str) -> str:
    return re.sub(r'([a-z0-9])([A-Z])', r'\1_\2', name).lower().removesuffix('0104').rstrip('_')


def _blocks(values, limit=MAX_MODULE_LINES):
    batch=[];size=0
    for value in values:
        lines=len(value[1].splitlines())
        if batch and size+lines>limit:
            yield batch;batch=[];size=0
        batch.append(value);size+=lines
    if batch:yield batch


def _frame_groups(text: str):
    """Only separate disjoint match arms; preserve each arm verbatim."""
    matches=list(re.finditer(r'^        (0x[0-9a-fA-F]+) => ',text,re.MULTILINE))
    fallback=text.index('        _ => return None,',matches[-1].end())
    groups=defaultdict(list)
    for i,m in enumerate(matches):
        end=matches[i+1].start() if i+1<len(matches) else fallback
        key=int(m.group(1),16)>>7
        groups[key].append(text[m.start():end])
    return groups


def partition_wire(source: str) -> dict[str,str]:
    """Return final paths relative to protocol/src; no intermediate file is written."""
    header,items=parse(source)
    output={};root=[header];record_groups=defaultdict(list);test_item=None
    i=0
    while i<len(items):
        item=items[i]
        if item.kind=='struct':
            name=item.name;pieces=[item.text];i+=1
            while i<len(items) and items[i].kind=='impl':
                pieces.append(items[i].text);i+=1
            text=''.join(pieces)
            packet=re.search(r'packet ID `0x([0-9a-f]+)`',text)
            family=('packets_'+packet.group(1)[:2]) if packet else 'records'
            record_groups[family].append((name,text));continue
        if item.kind=='mod' and item.name=='tests':
            test_item=item;i+=1;continue
        if item.kind=='fn' and item.name=='frame_kind':
            groups=_frame_groups(item.text)
            root.append('\n/// Wire shape from the original descriptor table.\n#[must_use]\n')
            root.append('pub fn frame_kind(packet_id: u32) -> Option<FrameKind0104> {\n    match packet_id >> 7 {\n')
            for key,arms in sorted(groups.items()):
                module=f'frames_{key:x}'
                root.append(f'        0x{key:x} => {module}::frame_kind(packet_id),\n')
                output[f'wire_0104/{module}.rs']=''.join([
                    '// Generated descriptor range; do not edit by hand.\nuse super::*;\n\n',
                    'pub(super) fn frame_kind(packet_id: u32) -> Option<FrameKind0104> {\n',
                    '    Some(match packet_id {\n',*arms,'        _ => return None,\n    })\n}\n'])
            root.append('        _ => None,\n    }\n}\n')
            for key in sorted(groups):root.append(f'\nmod frames_{key:x};\n')
        elif item.name in ('declared_struct_size','PACKET_IDS_0104','OPENFUSION_SIZE_DIVERGENCES_0104'):
            module={'declared_struct_size':'declared_sizes','PACKET_IDS_0104':'packet_names','OPENFUSION_SIZE_DIVERGENCES_0104':'server_differences'}[item.name]
            output[f'wire_0104/{module}.rs']='// Generated table; do not edit by hand.\nuse super::*;\n'+item.text.strip()+'\n'
            root.append(f'\nmod {module};\npub use {module}::{item.name};\n')
        else:root.append(item.text)
        i+=1
    for family,values in record_groups.items():
        for batch in _blocks(values):
            module=family+'_'+_snake(batch[0][0])
            output[f'wire_0104/{module}.rs']='// Generated wire layouts; do not edit by hand.\nuse super::*;\n'+''.join(text for _,text in batch).strip()+'\n'
            root.append(f'\nmod {module};\npub use {module}::*;\n')
    if test_item is not None:
        body=source[test_item.body_start:test_item.body_end]
        import textwrap
        body=textwrap.dedent(body).strip()+'\n'
        th,tests=parse(body);testroot=[th];cases=[]
        for case in tests:
            if case.kind=='fn' and '#[test]' in case.text:
                cases.append((case.name,case.text))
            else:testroot.append(case.text)
        for batch in _blocks(cases):
            module=_snake(batch[0][0])
            output[f'wire_0104/tests/{module}.rs']='use super::*;\n'+''.join(text for _,text in batch).strip()+'\n'
            testroot.append(f'\nmod {module};\n')
        output['wire_0104/tests.rs']=''.join(testroot).rstrip()+'\n'
        root.append('\n#[cfg(test)]\nmod tests;\n')
    output['wire_0104.rs']=''.join(root).rstrip()+'\n'
    return output
