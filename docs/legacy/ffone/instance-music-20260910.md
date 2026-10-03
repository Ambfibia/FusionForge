# Instance music after a warp

The reported symptom was the loading theme continuing inside infected zones
and portal destinations. Current Retrobution 20260821 is the primary authority.

## Cause and native contract

`GameFrame.Update` sends `P_CL2FE_REQ_PC_LOADING_COMPLETE` with `iPC_ID = 0`
on the transition from a non-ready DongLoader to a ready DongLoader. This is
not limited to the initial login. `GameFrame` receives `INSTANCE_MAP_INFO`
and sets `bInsMap = true` even when the instance has no race (`iEP_ID = 0`).
`WorldDataContainer` passes that flag and the player's Unity X/Z coordinates
to `MusicController.LoadSoundByCoordinate`.

OpenFusion's `PlayerManager::loadPlayer` sends `INSTANCE_MAP_INFO` after
`LOADING_COMPLETE` for every non-overworld instance. `sendPlayerTo` sends the
warp coordinates, but does not itself send the instance notification.

FFOne completed this handshake during initial login only. Its authoritative
teleport path rearmed local collider and presentation loading, but never sent
the completion notification. As a result the music selector retained the
outdoor flag and could not select the installed infected-zone/lair polygons.

The native repair marks the teleported local player as awaiting a loading
acknowledgment. After collider/presentation admission and the loading screen
closing, it enqueues the existing registered protocol-0104 request once.
Duplicate NPC-warp/GOTO packets share the same marker; a queue failure retains
it for retry; menu states cannot send it. The marker belongs to the player and
is removed with session teardown. Initial entry retains its network-worker
handshake. Instance identity continues to come from the server reply.

No audio payload or zone polygon was changed. The production zone regression
checks Pokey Oaks' infected-zone music and the existing
`ambient/ambient_fusion_lairs` bed selected in fusion lairs. Some lairs request
silence on the music channel and carry their ominous background on the ambient
channel; that separation is intentional.

## Evidence and verification

See `docs/reference/evidence/cases/instance-music-20260910.json` for hashes, replay commands,
and verification results. Raw decompilation remains in
`work/cases/instance-music-20260910`.

This is a loading/authority routing correction. Automated ECS selection and
request checks do not constitute an audible Unity/native comparison. The
four previously unresolved music routes in the old zone document remain
outside this repair.
