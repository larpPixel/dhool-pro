#!/usr/bin/env sh
set -eu

# Local-only importer for a licensed Counter-Strike 2 install. It exports the
# real de_dust2 visual map, material textures, skybox, and compatibility
# collision/physics mesh through ValveResourceFormat's Source2Viewer-CLI into
# assets/dust2/generated/cs2/, which is git-ignored.

root=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
cs2_game=${CS2_GAME:-/mnt/windows/Program Files (x86)/Steam/steamapps/common/Counter-Strike Global Offensive/game}
vrf=${SOURCE2VIEWER_CLI:-Source2Viewer-CLI}
case "$vrf" in
    */*) vrf=$(CDPATH= cd -- "$(dirname -- "$vrf")" && pwd)/$(basename -- "$vrf") ;;
esac
staging=${TMPDIR:-/tmp}/dhool-cs2-dust2-import
sky_staging=${TMPDIR:-/tmp}/dhool-cs2-dust2-sky-import
output=$root/assets/dust2/generated/cs2
map_vpk=$cs2_game/csgo/maps/de_dust2.vpk
pak_vpk=$cs2_game/csgo/pak01_dir.vpk
threads=${VRF_THREADS:-4}

if [ ! -f "$map_vpk" ]; then
    echo "missing CS2 Dust2 VPK: $map_vpk" >&2
    exit 2
fi

if [ ! -f "$pak_vpk" ]; then
    echo "missing CS2 pak VPK: $pak_vpk" >&2
    exit 2
fi

rm -rf "$staging" "$sky_staging" "$output/maps" "$output/materials/skybox"
mkdir -p "$staging" "$sky_staging" "$output/maps/de_dust2" "$output/materials/skybox"

"$vrf" \
    -i "$map_vpk" \
    -o "$staging" \
    --vpk_decompile \
    --gltf_export_format glb \
    --gltf_export_materials \
    --gltf_textures_adapt \
    --texture_decode_flags ForceLDR \
    --threads "$threads" \
    -e vmap_c \
    -f maps/de_dust2.vmap_c

cp "$staging/maps/de_dust2.glb" "$output/maps/de_dust2.glb"
find "$staging/maps" -maxdepth 1 -type f \( -name '*.png' -o -name '*.jpg' -o -name '*.jpeg' \) \
    -exec cp {} "$output/maps/" \;

"$vrf" \
    -i "$map_vpk" \
    -o "$staging" \
    --vpk_decompile \
    --gltf_export_format glb \
    --threads "$threads" \
    -e vmdl_c \
    -f maps/de_dust2/world_physics.vmdl_c

cp "$staging/maps/de_dust2/world_physics_physics.glb" \
    "$output/maps/de_dust2/world_physics_physics.glb"

(cd "$sky_staging" && "$vrf" \
    -i "$pak_vpk" \
    -o "$sky_staging" \
    --vpk_decompile \
    --texture_decode_flags ForceLDR \
    --threads "$threads" \
    -e vmat_c,vtex_c \
    -f materials/skybox/sky_de_dust2)
rm -f "$sky_staging/exceptions.txt"

if [ -f "$sky_staging/materials/skybox/sky_de_dust2.vmat" ]; then
    cp "$sky_staging/materials/skybox/sky_de_dust2.vmat" "$output/materials/skybox/"
fi
if [ -f "$sky_staging/materials/skybox/sky_de_dust2.exr" ]; then
    cp "$sky_staging/materials/skybox/sky_de_dust2.exr" "$output/materials/skybox/"
	command -v magick >/dev/null 2>&1 || {
		echo "ImageMagick is required to convert the Dust2 skybox" >&2
		exit 1
	}
	magick "$sky_staging/materials/skybox/sky_de_dust2.exr" \
		-resize 2048x1024 -colorspace sRGB -alpha off \
		"$output/materials/skybox/sky_de_dust2.png"
fi

test -s "$output/materials/skybox/sky_de_dust2.vmat" || {
	echo "Dust2 skybox material export is missing" >&2
	exit 1
}
test -s "$output/materials/skybox/sky_de_dust2.png" || {
	echo "Dust2 skybox panorama export is missing" >&2
	exit 1
}

visual_sha256=$(sha256sum "$output/maps/de_dust2.glb" | cut -d ' ' -f 1)
physics_sha256=$(sha256sum "$output/maps/de_dust2/world_physics_physics.glb" | cut -d ' ' -f 1)
texture_count=$(find "$output/maps" -maxdepth 1 -type f \( -name '*.png' -o -name '*.jpg' -o -name '*.jpeg' \) | wc -l | tr -d ' ')
cat > "$output/manifest.json" <<EOF
{
  "source": "$map_vpk",
  "shared_source": "$pak_vpk",
  "exporter": "ValveResourceFormat Source2Viewer-CLI",
  "visual_mesh": "maps/de_dust2.glb",
  "visual_sha256": "$visual_sha256",
  "visual_texture_count": $texture_count,
  "physics_mesh": "maps/de_dust2/world_physics_physics.glb",
  "physics_sha256": "$physics_sha256",
  "skybox_material": "materials/skybox/sky_de_dust2.vmat",
  "skybox_texture": "materials/skybox/sky_de_dust2.png",
  "note": "Local licensed CS2-derived Dust2 visual map, textures, skybox, and physics mesh; ignored by git."
}
EOF

echo "wrote $output/maps/de_dust2.glb"
echo "wrote $texture_count material textures under $output/maps"
echo "wrote $output/maps/de_dust2/world_physics_physics.glb"
