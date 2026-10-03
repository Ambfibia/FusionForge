use super::*;

pub(super) fn runtime_ambience_contract(scope: TerrainScope) -> JsonValue {
    json!({
        "registration": {
            "source": "DongColorSetup.Start",
            "operation": "DongLoader.SetDongColors(x, y, fogDepth, fogColor, skyColor, lightColor)",
            "gridWidth": 16,
            "gridIndexFormula": "x + 16 * y",
        },
        "sampling": {
            "source": "DongLoader.GetDongColors",
            "localCoordinates": {
                "x": "playerPosition.x / 512 - 0.5",
                "y": "playerPosition.z / 512 - 0.5",
            },
            "neighborSelection": "floor(localCoordinate) and +1 on each axis",
            "fractionRemap": "clamp01((fraction - 0.25) * 2)",
            "weights": [
                "(1 - tx) * (1 - ty)",
                "tx * (1 - ty)",
                "(1 - tx) * ty",
                "tx * ty"
            ],
            "missingTilePolicy": "a neighbor contributes only when HasDongColors is true",
            "normalization": "divide accumulated fogDepth/fogColor/skyColor/lightColor by total contributing weight",
            "zeroWeightFallback": {
                "fogDepth": 0.0,
                "fogColor": [0.0, 0.0, 0.0, 0.0],
                "skyColor": [1.0, 1.0, 1.0, 1.0],
                "lightColor": [1.0, 1.0, 1.0, 1.0],
            },
        },
        "defaultAmbienceApplication": {
            "source": "cnPlayerCamera.DefaultAmbience",
            "lightColor": "sampledLightColor * 0.6 + white * 0.4",
            "fogDensity": "sampledFogDepth * 0.005",
            "fogEnabled": "sampledFogDepth > 0",
            "fogColor": "sampledFogColor",
            "tutorialFogColor": "sampledFogColor * 0.5 + white * 0.5",
            "tutorialBlendAppliesToThisScope": scope == TerrainScope::Tutorial,
            "fogAlphaAfterScopeBlend": 0.85,
        },
        "runtimePolicy": "evaluate continuously from player position; do not bake one interpolated value per tile",
    })
}
