"""Re-sort assets/game/audio/sfx by meaning and move performed speech into voice.

The SFX tree was imported by legacy package, not by meaning: `sfx/shared` became
a 2162-file catch-all holding creature barks, interface clicks, world ambience
and named-character dialogue side by side, while `sfx/environment` collected
three monsters whose names happen to start with "fire".

This rewrites both halves of a route at once -- the file on disk and its row in
`native_asset_routes.m_pAudioData` -- because the catalogue convention is that
the logical key mirrors the file's place. `trueName` is never rewritten: it is
the exact Unity source name and the runtime resolves clips by it. The previous
logical key is kept as an alias so `by_legacy_path` still resolves old routes.

Layout produced:

    sfx/creatures/<monster>/[<scope>/]<cue>.ogg   monster clips, grouped per monster
    sfx/characters/<name>/<cue>.ogg               named characters' non-vocal foley

A `zone_local`/`world_shared`/`npc_pack`/`tutorial_audio` segment is preserved
for a creature, whose clips stay SFX either way, but not for a performance: the
scope says which legacy bundle a copy shipped in, and the copies are the same
recording, so a character's line resolves to one voice row and the rest are
dropped as the duplicates they are.
    sfx/combat/                                   weapon impacts and generic combat
    sfx/movement/                                 player locomotion
    sfx/vehicles/  sfx/ui/  sfx/nano/
    sfx/environment/                              world ambience emitters
    sfx/world_events/                             stings, warps, explosions, quakes
    voice/en/<speaker>/<cue>.ogg                  performed speech

    python tools/legacy-sources/resort-sfx-by-owner.py --native-target ../FFOneClient/assets/game
    ... --apply
"""

from __future__ import annotations

import argparse
import collections
import hashlib
import json
import os
import re
import shutil
import sys

ASSETS = r'D:\CodexProject\FusionFallProject\FFOneClient\assets\game'
TS = os.path.join(ASSETS, 'data', 'tables', 'table-set.json')

# ---------------------------------------------------------------- cue grammar

CUES = [
    'corruptak_readyspell', 'megatak_readyspell', 'corruptakreadyspell',
    'megatakreadyspell', 'megatakready', 'readyspell',
    'corruptak', 'curruptak', 'megatak',
    'attackgrunt', 'attackwhoosh', 'weapontargetsfx', 'weaponsfx', 'swordsfx',
    'gruntshrt', 'grunt',
    'melee', 'range', 'attack', 'atkshrt', 'atk', 'skill', 'skil',
    'dodge', 'critical', 'wound', 'death', 'defeat', 'hurt', 'ready',
    'standup', 'stand', 'idle',
    'walk', 'running', 'run', 'landing', 'jumpstart', 'jumplnd', 'jump',
    'appear', 'warp', 'sting',
]
CUE_RE = re.compile(r'^(?P<stem>.+?)_(?P<cue>' + '|'.join(CUES) + r')(?P<tail>[0-9a-z_]*)$')

# Cues that are the character's own voice. Everything else a character owns is
# foley -- footsteps, a sword swing, a weapon's report -- and stays under sfx.
VOCAL_CUES = {
    'attackgrunt', 'grunt', 'gruntshrt',
    'stand', 'standup', 'wound', 'death', 'defeat', 'hurt', 'critical',
    'ready', 'readyspell', 'corruptak', 'curruptak', 'megatak',
    'corruptak_readyspell', 'megatak_readyspell', 'corruptakreadyspell',
    'megatakreadyspell', 'megatakready',
    'skill', 'skil', 'atk', 'atkshrt', 'attack',
}

# Named characters whose clips are a performance. Fusion clones are voiced by
# the same actors as the originals, so they are speakers in their own right.
# Every non-Fusion name here already owns a voice folder.
ROSTER = {
    'ben': 'ben', 'billy': 'billy', 'blossom': 'blossom', 'dexter': 'dexter',
    'kevin': 'kevin', 'vilgax': 'vilgax', 'dukey': 'dukey', 'numfive': 'numfive',
    'numbuhfive': 'numfive', 'numbuhone': 'numbuhone', 'samjack': 'samjack',
    'buttercup': 'buttercup', 'bubbles': 'bubbles', 'm_chickn1': 'm_chickn1',
    'iceking': 'iceking', 'gunter': 'gunter',
}
# Every Fusion clone is a speaker in its own right, so the roster is derived
# from the stem rather than listed by hand. Two are excluded:
#   fusionhominid  -- a re-skinned Lion Gorilla; 13 of its 17 clips are that
#                     monster's audio byte-for-byte, so it is sound design;
#   fusionblowfish -- a blowfish, not a character.
FUSION_EXCLUDED = {'fusionhominid', 'fusionblowfish'}
# Fuse himself and his limbs stay creatures: the boss's clips are roars and the
# arm entries are pure sound design.
FUSION_PREFIX = 'fusion'
# The source spells three of them with a separator. `Fusion_Ed` and `FusionEd`
# are one character split across two spellings with complementary cues, the
# same case as Mandroid's `m_mandroid1`/`m_mandrd1`, so they share an owner.
FUSION_SPELLINGS = {
    'fusion_computress': 'fusioncomputress',
    'fusion_courage': 'fusioncourage',
    'fusion_ed': 'fusioned',
}


def fusion_owner(stem):
    stem = FUSION_SPELLINGS.get(stem, stem)
    if not stem.startswith(FUSION_PREFIX) or stem in FUSION_EXCLUDED:
        return None
    return stem

# ------------------------------------------------------- explicit name routing

UI = set("""
abandon_mission action_failure01 action_failure02 action_sucess add_attribute
affirmation_screen_popup character_limit_max close_screen clothing_equipped
clothing_unequipped comm_slidein comm_slideout container_open01 container_open02
continue_01 drag_item_failed egg_pickup email_arrived girth_narrow girth_wide
height_down height_up incoming_tell inventory_full_alert invenlooping
mission_accepted mission_completed mission_decline money_misc01 money_misc02
money_misc03 money_transfer01 money_transfer02 mouse_click01 mouse_click02
mouse_click03 mouse_click04 mouse_click05 name_arrows nano_equipped
nano_unequipped open_screen outgoing_chat pickup_pickup1 purchase01 purchase02
randomize ring scroll_down scroll_up select_color system_announcement_alert
tab_click01 task_completed treasure_chest_open zoom_in zoom_out crocpot_fail
crocpot_inmakeout crocpot_success loseeffect wineffect tieeffect racestart
racefinish timer_race nano_machine_clickon click_windowslideout launcher_clickon
launcher_firing launcher_powerpulse launcher_startpower launcher_stoppower
weapon_equipped weapon_unequipped back_button next_button no_button yes_button
""".split())

NANO = set("""
nano_ability_06 nano_creation nano_creation_complete nano_creation_failure
nano_failsummon nano_potion nano_terminal_1 nano_terminal_2 nano_terminal_3
""".split())

VEHICLES = set("""
vehicle_cloud1 vehicle_cloud2 vehicle_cloud3 vehicle_cloud4 vehicle_getoff
vehicle_geton vehicle_hover1 vehicle_hover2 vehicle_hover3 vehicle_hover4
vehicle_hovercar vehicle_scooter1 vehicle_scooter2 vehicle_scooter3
vehicle_scooter4 vehicle_timesquad vehicle_ufo scamper_jet vehicle_jumpstart
vehicle_jumplnd
""".split())

MOVEMENT = set("""
avatar_flying avatar_swimback avatar_swimidle landing sfx_landing_water
""".split())

WORLD_EVENTS = set("""
big_eruption black_hole buddy_warp collapse_tower crater dexbot_warp explosion
explosion_01 explosion_02 explosion_03 explosion_04 fissure_terrafuser
fusion_crater fusion_egg fusionlair_warp fusionmatter fusionmelt
laircollapse_quake_loop recall_warp resurrectm_warp rumblequake_loop
small_eruption teleporter transportation_warp fusion_lair_teleporter
thunderous_crash poison_damage sfx_poisondamage sfx_bouncetech sfx_zipline_final
sfx_emergewater bouncetoon cyberus_sting dexcarrier_sting flythru_sting
infectedzone_sting planetfusion_sting scamper_sting techwing_sting
fusionspawns_sting cyberus_landing
""".split())

ENVIRONMENT = set("""
big_antenna biocontainers_1 biocontainers_2 bubble_creator bubbie cactus car_car1
communicatons_tower_1 communicatons_tower_2 communicatons_tower_3 dark_tree
deformed_tree dexlabs_airship dexterhologram_loop dexters_house_1 drainage
elevators_mojo_s_volcano extreme_o_saurus_cage flag_tent_1 flag_tent_2
flowing_fountain fuse_lair fusion_bubbles fusion_drainage fusion_flow
fusion_matter_puddle fusion_pool gnats great_machine_1 greater_terrafuser_1
greater_terrafuser_2 healing_pool holograms_1 holograms_2 holograms_3 hologramoff
hologramon knd_treehouse_1_best mechwhale mermaid_fountain moving_sculpture
moving_sculpture_2 neon_sign_1 neon_sign_2 ocean_surf pirate_ship_wharf
pistons_genius_grove pond_1 pond_2 pond_3 pool_1 pool_2 portal_amusement_park
railway_transportation_system ressurectum_1 ressurectum_2_best river
serpent_breath sound_stage_mall spore_plant spraying_fountain_1
spraying_fountain_2 taros tendril terrafuser_1 terrafuser_2 time_pod tv_noise
woodbridge warning_siren bigfire fire_1 fire_2 fire_plane fire_stone
water_cleaner waterfall waterway machine_running techsquaretext_typed
""".split())

COMBAT = set("""
01_sword_impact 02_rifle_impact 03_fire_impact 04_ice_impact 05_water_impact
megaattackarea megaattackareahittype megaattackmissile megaattackspell
corruptionatypecasting corruptionbtypecasting corruptionctypecasting
""".split())

# Weapon-type hit families: <material><weight>(user|target)_NN.
IMPACT_RE = re.compile(
    r'^(electric|explos[a-z]*|fire|ice|laser[a-z]*|melee[a-z]*|shell[a-z]*|'
    r'slime[a-z]*|sonic|thrown|torpedo)(user|target)_\d+$'
)

# Names whose stem/cue split the grammar cannot see, because the source spelled
# them without a separator.
EXPLICIT = {
    'plyravtf_attack1': ('voice', 'f_avatar', 'avatar_attack1'),
    'plyravtf_attack1upper': ('voice', 'f_avatar', 'avatar_attack1upper'),
    'plyravtf_attack2upper': ('voice', 'f_avatar', 'avatar_attack2upper'),
    'bcupattack1': ('voice', 'buttercup', 'attack1'),
    'bcupattack2': ('voice', 'buttercup', 'attack2'),
    'bcupattack3': ('voice', 'buttercup', 'attack3'),
    'bcupattack4': ('voice', 'buttercup', 'attack4'),
    'bcupattack5': ('voice', 'buttercup', 'attack5'),
    'icekingmelee1': ('characters', 'iceking', 'melee1'),
    'icekingmelee2': ('characters', 'iceking', 'melee2'),
    'icekingmelee3': ('characters', 'iceking', 'melee3'),
    'icekingdeath': ('voice', 'iceking', 'death'),
    'icekingwound1': ('voice', 'iceking', 'wound1'),
    'icekingwound2': ('voice', 'iceking', 'wound2'),
    'icekinggoodluck': ('voice', 'iceking', 'goodluck'),
    'icekingqgreet': ('voice', 'iceking', 'qgreet'),
    'ice_comm01': ('voice', 'iceking', 'comm01'),
    'ice_comm02': ('voice', 'iceking', 'comm02'),
    'ice_comm03': ('voice', 'iceking', 'comm03'),
    'gunter1': ('voice', 'gunter', 'call1'),
    'gunter2': ('voice', 'gunter', 'call2'),
    'gunter3': ('voice', 'gunter', 'call3'),
    'gunterwound1': ('voice', 'gunter', 'wound1'),
    'gunterwound2': ('voice', 'gunter', 'wound2'),
    'gunterwound3': ('voice', 'gunter', 'wound3'),
    'gunterwalk': ('characters', 'gunter', 'walk'),
    'gunterrun': ('characters', 'gunter', 'run'),
    'numfive_nice01': ('voice', 'numfive', 'nice01'),
    'dukey_vndfarewell01': ('voice', 'dukey', 'vndfarewell01'),
    'dukey_vndfarewell02': ('voice', 'dukey', 'vndfarewell02'),
    'dukey_vndfarewell03': ('voice', 'dukey', 'vndfarewell03'),
    'dukey_vndgreeting01': ('voice', 'dukey', 'vndgreeting01'),
    'dukey_vndgreeting02': ('voice', 'dukey', 'vndgreeting02'),
    'dukey_vndgreeting03': ('voice', 'dukey', 'vndgreeting03'),
    'bubbles_lasereyes_long': ('characters', 'bubbles', 'lasereyes_long'),
    'buttercup_flyby_whoosh': ('characters', 'buttercup', 'flyby_whoosh'),
    'ghost_s_f1_a': ('creatures', 'ghost_s', 'f1_a'),
    'ghost_s_f1_b': ('creatures', 'ghost_s', 'f1_b'),
    'ghostduck_appear01': ('creatures', 'ghostduck', 'appear01'),
    'ghostduck_appear02': ('creatures', 'ghostduck', 'appear02'),
    'ghostduck_appear03': ('creatures', 'ghostduck', 'appear03'),
    'ghostduck_appear04': ('creatures', 'ghostduck', 'appear04'),
    'ghostduck_appear05': ('creatures', 'ghostduck', 'appear05'),
    'lobster_fish_idle': ('creatures', 'lobster_fish', 'idle'),
    'lobster_fish_attack_1': ('creatures', 'lobster_fish', 'attack_1'),
    'lobster_fish_attack_2': ('creatures', 'lobster_fish', 'attack_2'),
    'lobster_fish_death': ('creatures', 'lobster_fish', 'death'),
    'oilmonster_oo_standup': ('creatures', 'oilmonster_oo', 'standup'),
    'crocpot_stand1': ('creatures', 'crocpot', 'stand1'),
    'crocpot_stand2': ('creatures', 'crocpot', 'stand2'),
    'm_chickn1_gruntshrt_01': ('voice', 'm_chickn1', 'gruntshrt_01'),
}

# Scope segments the runtime filters on; they must survive in the logical key.
SCOPES = ('zone_local', 'world_shared', 'tutorial_audio', 'npc_pack')

KEEP_OWNERS = ('character_creation', 'nano_skills')


# ------------------------------------------------------------------- payloads

def payload_hash(path):
    """Hash Ogg packet bodies, skipping the per-stream serial and page CRCs."""
    data = open(path, 'rb').read()
    h = hashlib.blake2b(digest_size=16)
    i, n = 0, len(data)
    while i + 27 <= n:
        if data[i:i + 4] != b'OggS':
            h.update(data[i:])
            break
        segs = data[i + 26]
        tbl = i + 27
        if tbl + segs > n:
            h.update(data[i:])
            break
        start = tbl + segs
        end = start + sum(data[tbl:tbl + segs])
        h.update(data[start:end])
        i = end
    return h.hexdigest()


# -------------------------------------------------------------------- routing

def route(row):
    """Return (tree, group, cue) for an sfx row, or None to leave it alone."""
    key = row['logicalKey']
    parts = key.split('/')
    name = parts[-1]
    scope = next((p for p in parts[2:-1] if p in SCOPES), None)
    if row['owner'] in KEEP_OWNERS:
        return None
    if name in EXPLICIT:
        tree, group, cue = EXPLICIT[name]
        return tree, group, cue, scope
    if name in UI:
        return 'flat', 'ui', name, None
    if name in NANO:
        return 'flat', 'nano', name, None
    if name in VEHICLES:
        return 'flat', 'vehicles', name, None
    if name in MOVEMENT:
        return 'flat', 'movement', name, None
    if name in WORLD_EVENTS:
        return 'flat', 'world_events', name, None
    if name in ENVIRONMENT:
        return 'flat', 'environment', name, None
    if name in COMBAT or IMPACT_RE.match(name):
        return 'flat', 'combat', name, None
    m = CUE_RE.match(name)
    if not m:
        return None
    stem, cue, tail = m.group('stem'), m.group('cue'), m.group('tail')
    full_cue = cue + tail
    owner = ROSTER.get(stem) or fusion_owner(stem)
    if owner is not None and cue in VOCAL_CUES:
        # The scope a clip shipped in is a packaging fact about the legacy
        # bundle, not about the clip. Kevin's defeat cry is his performance
        # whether it arrived in `world_shared` or `zone_local`, and the two
        # copies are the same recording, so both resolve to one voice line.
        return 'voice', owner, full_cue, None
    if owner is not None:
        return 'characters', owner, full_cue, scope
    return 'creatures', stem, full_cue, scope


def plan_paths(tree, group, cue, scope):
    tail = f'{scope}/{cue}' if scope else cue
    if tree == 'voice':
        return f'audio/voice/en/{group}/{tail}.ogg', f'voice/{group}/{tail}', group
    if tree == 'flat':
        return f'audio/sfx/{group}/{tail}.ogg', f'sfx/{group}/{tail}', group
    return (f'audio/sfx/{tree}/{group}/{tail}.ogg',
            f'sfx/{tree}/{group}/{tail}', f'{tree}/{group}')


# ----------------------------------------------------------------------- main

def main():
    global ASSETS, TS
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument('--native-target', required=True,
                    help="the client's assets/game directory")
    ap.add_argument('--apply', action='store_true')
    ap.add_argument('--report', default='sfx-resort-report.json')
    args = ap.parse_args()
    ASSETS = os.path.abspath(args.native_target)
    TS = os.path.join(ASSETS, 'data', 'tables', 'table-set.json')

    # A one-way move out of the legacy layout. Re-running over the published
    # tree would re-split names that are already cues, so it stops here rather
    # than relying on the collision check to catch it.
    if not os.path.isdir(os.path.join(ASSETS, 'audio', 'sfx', 'shared')):
        raise SystemExit('audio/sfx/shared is absent: this tree is already re-sorted')

    doc = json.loads(open(TS, encoding='utf-8').read())
    routes = next(t for t in doc['tables'] if t['name'] == 'native_asset_routes')
    rows = routes['value']['m_pAudioData']

    voice_rows = [r for r in rows if r['category'] == 'voice']
    sfx_rows = [r for r in rows if r['category'] == 'sfx']

    # Existing voice clips, indexed by payload, so an sfx row that is the same
    # recording as a published line is recognised as a duplicate rather than a
    # second take. Raw file hashes miss these: the two exports differ only in
    # Ogg stream serial and page CRC.
    voice_payload = {}
    for r in voice_rows:
        p = os.path.join(ASSETS, 'audio', 'voice', 'en', *r['path'].split('/'))
        if os.path.isfile(p):
            voice_payload.setdefault(payload_hash(p), r)

    moves, drops, untouched, unrouted = [], [], [], []
    for row in sfx_rows:
        r = route(row)
        if r is None:
            (untouched if row['owner'] in KEEP_OWNERS else unrouted).append(row)
            continue
        tree, group, cue, scope = r
        new_path, new_key, owner = plan_paths(tree, group, cue, scope)
        src = os.path.join(ASSETS, *row['path'].split('/'))
        if not os.path.isfile(src):
            raise SystemExit(f'missing source file {row["path"]}')
        if tree == 'voice':
            dup = voice_payload.get(payload_hash(src))
            # A duplicate is the same recording under the same speaker AND the
            # same Unity true name. Sharing only the recording is not enough:
            # `PlyrAvtF_Attack1` and `M_Avatar_Attack1` are the female and male
            # avatar, `Dukey_VndGreeting01` and `Dukey_Greeting01` are a vendor
            # line and a world line, and two Fusion Frankie clips sit in a Van
            # Kleiss nano folder as `..._copy`. The true name is the runtime's
            # identity, so folding any of those together would lose a cue.
            if (dup is not None and dup['owner'] == group
                    and dup['trueName'].strip().lower()
                    == row['trueName'].strip().lower()):
                drops.append({'row': row, 'src': row['path'], 'into': dup,
                              'intoKey': dup['logicalKey']})
                continue
            new_path = f'audio/voice/en/{group}/{cue}.ogg'
        if new_path == row['path'] and new_key == row['logicalKey']:
            untouched.append(row)
            continue
        moves.append({'row': row, 'from': row['path'], 'to': new_path,
                      'fromKey': row['logicalKey'], 'toKey': new_key,
                      'owner': owner, 'category': 'voice' if tree == 'voice' else 'sfx'})

    # ------------------------------------------------------------- validation
    problems = []
    taken_paths = {r['path'] for r in rows if r['category'] != 'sfx'}
    taken_paths |= {f'audio/voice/en/{r["path"]}' for r in voice_rows}
    taken_keys = {r['logicalKey'].lower() for r in rows}
    for m in moves:
        taken_keys.discard(m['fromKey'].lower())
    for d in drops:
        taken_keys.discard(d['row']['logicalKey'].lower())
    seen_p, seen_k = {}, {}
    for m in moves:
        if m['to'] in taken_paths:
            problems.append(f'target path already in use: {m["to"]}')
        if m['to'] in seen_p:
            problems.append(f'two rows target {m["to"]} ({seen_p[m["to"]]}, {m["fromKey"]})')
        seen_p[m['to']] = m['fromKey']
        lk = m['toKey'].lower()
        if lk in taken_keys:
            problems.append(f'target key already in use: {m["toKey"]}')
        if lk in seen_k:
            problems.append(f'two rows target key {m["toKey"]}')
        seen_k[lk] = m['fromKey']
        dest = os.path.join(ASSETS, *m['to'].split('/'))
        if os.path.exists(dest):
            problems.append(f'destination exists on disk: {m["to"]}')

    # voice take sets: one primary per trueName, alternates marked
    groups = collections.defaultdict(list)
    for r in voice_rows:
        groups[r['trueName'].strip().lower()].append(r.get('scope'))
    for m in moves:
        if m['category'] == 'voice':
            groups[m['row']['trueName'].strip().lower()].append(m['row'].get('scope'))
    for tn, scopes in groups.items():
        if len(scopes) > 1 and sum(1 for s in scopes if s != 'alternate_take') != 1:
            problems.append(f'voice take set {tn!r} would have '
                            f'{sum(1 for s in scopes if s != "alternate_take")} primaries')

    print(f'move {len(moves)}  drop-as-duplicate {len(drops)}  '
          f'unchanged {len(untouched)}  unrouted {len(unrouted)}')
    by_dest = collections.Counter(
        ('voice/' + m['owner']) if m['category'] == 'voice'
        else '/'.join(m['to'].split('/')[1:3]) for m in moves)
    for k, v in sorted(by_dest.items(), key=lambda kv: (-kv[1], kv[0]))[:25]:
        print(f'  {v:5}  {k}')
    if unrouted:
        print(f'--- {len(unrouted)} unrouted ---')
        for r in unrouted[:40]:
            print('   ', r['logicalKey'])
    if problems:
        print(f'--- {len(problems)} problems ---')
        for p in problems[:40]:
            print('   ', p)
        raise SystemExit('refusing to apply')

    report = {
        'schema': 'ffone.audio-resort.v1',
        'applied': bool(args.apply),
        'counts': {'moved': len(moves), 'droppedDuplicates': len(drops),
                   'unchanged': len(untouched), 'unrouted': len(unrouted)},
        'byDestination': dict(sorted(by_dest.items())),
        'droppedDuplicates': [{'key': d['row']['logicalKey'], 'path': d['src'],
                               'sameAs': d['intoKey']} for d in drops],
        'moves': [{'trueName': m['row']['trueName'], 'from': m['from'],
                   'to': m['to'], 'toKey': m['toKey'],
                   'category': m['category']} for m in moves],
    }
    with open(args.report, 'w', encoding='utf-8') as fh:
        json.dump(report, fh, indent=1, ensure_ascii=False)
        fh.write('\n')

    if not args.apply:
        print(f'plan only; wrote {args.report}')
        return 0

    for d in drops:
        os.remove(os.path.join(ASSETS, *d['src'].split('/')))
        rows.remove(d['row'])
        target = d['into']
        target['aliases'] = sorted(set(target.get('aliases', []))
                                   | {d['row']['logicalKey']})
    for m in moves:
        src = os.path.join(ASSETS, *m['from'].split('/'))
        dst = os.path.join(ASSETS, *m['to'].split('/'))
        os.makedirs(os.path.dirname(dst), exist_ok=True)
        shutil.move(src, dst)
        row = m['row']
        row['logicalKey'] = m['toKey']
        row['owner'] = m['owner']
        row['aliases'] = sorted(set(row.get('aliases', [])) | {m['fromKey']})
        if m['category'] == 'voice':
            row['category'] = 'voice'
            row['path'] = m['to'][len('audio/voice/en/'):]
        else:
            row['path'] = m['to']

    # Keep each row's field order stable and put aliases next to the key.
    order = ['logicalKey', 'aliases', 'trueName', 'category', 'owner', 'scope', 'path']
    routes['value']['m_pAudioData'] = [
        {k: r[k] for k in order if k in r} for r in
        sorted(rows, key=lambda r: (r['category'], r['logicalKey']))
    ]

    # Drop directories the move emptied.
    for base in ('audio/sfx', 'audio/voice'):
        root = os.path.join(ASSETS, *base.split('/'))
        for dirpath, dirnames, filenames in os.walk(root, topdown=False):
            if dirpath != root and not os.listdir(dirpath):
                os.rmdir(dirpath)

    tmp = TS + '.next'
    with open(tmp, 'w', encoding='utf-8', newline='\n') as fh:
        fh.write(json.dumps(doc, indent=2, ensure_ascii=False))
        fh.write('\n')
    os.replace(tmp, TS)
    print(f'applied; wrote {args.report} and {TS}')
    return 0


if __name__ == '__main__':
    sys.exit(main())
