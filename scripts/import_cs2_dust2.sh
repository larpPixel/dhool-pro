#!/usr/bin/env sh
set -eu

# Local-only importer for a licensed Counter-Strike 2 install. It exports the
# real de_dust2 world collision/physics mesh through ValveResourceFormat's
# Source2Viewer-CLI into assets/dust2/generated/cs2/, which is git-ignored.

root=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
cs2_game=${CS2_GAME:-/mnt/windows/Program Files (x86)/Steam/steamapps/common/Counter-Strike Global Offensive/game}
vrf=${SOURCE2VIEWER_CLI:-Source2Viewer-CLI}
staging=${TMPDIR:-/tmp}/dhool-cs2-dust2-import
output=$root/assets/dust2/generated/cs2
map_vpk=$cs2_game/csgo/maps/de_dust2.vpk

if [ ! -f "$map_vpk" ]; then
    echo "missing CS2 Dust2 VPK: $map_vpk" >&2
    exit 2
fi

rm -rf "$staging"
mkdir -p "$staging" "$output/maps/de_dust2"

"$vrf" \
    -i "$map_vpk" \
    -o "$staging" \
    --vpk_decompile \
    --gltf_export_format glb \
    -e vmdl_c \
    -f maps/de_dust2/world_physics.vmdl_c

cp "$staging/maps/de_dust2/world_physics_physics.glb" \
    "$output/maps/de_dust2/world_physics_physics.glb"

sha256=$(sha256sum "$output/maps/de_dust2/world_physics_physics.glb" | cut -d ' ' -f 1)
cat > "$output/manifest.json" <<EOF
{
  "source": "$map_vpk",
  "exporter": "ValveResourceFormat Source2Viewer-CLI",
  "mesh": "maps/de_dust2/world_physics_physics.glb",
  "sha256": "$sha256",
  "note": "Local licensed CS2-derived Dust2 physics/world collision mesh; ignored by git."
}
EOF

echo "wrote $output/maps/de_dust2/world_physics_physics.glb"
