#!/usr/bin/env sh
set -eu

root=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
destination="$root/assets/aaa"
api="https://api.polyhaven.com/files"
temporary=$(mktemp)
trap 'rm -f "$temporary"' EXIT

command -v curl >/dev/null
command -v jq >/dev/null

download() {
    download_url=$1
    download_target=$2
    if [ ! -s "$download_target" ]; then
        curl --fail --location --silent --show-error "$download_url" -o "$download_target"
    fi
}

fetch_model() {
    id=$1
    target="$destination/models/$id"
    mkdir -p "$target"
    curl --fail --location --silent --show-error "$api/$id" -o "$temporary"
    jq -r '.gltf["1k"].gltf.url' "$temporary" | while IFS= read -r url; do
        download "$url" "$target/${url##*/}"
    done
    jq -r '.gltf["1k"].gltf.include | to_entries[] | [.key, .value.url] | @tsv' "$temporary" |
        while IFS='	' read -r relative url; do
            mkdir -p "$target/$(dirname "$relative")"
            download "$url" "$target/$relative"
        done
}

fetch_surface() {
    id=$1
    target="$destination/surfaces/$id"
    mkdir -p "$target"
    curl --fail --location --silent --show-error "$api/$id" -o "$temporary"
    for map in Diffuse nor_gl arm; do
        url=$(jq -r --arg map "$map" '.[$map]["1k"].jpg.url' "$temporary")
        download "$url" "$target/$map.jpg"
    done
}

for model in Barrel_01 street_lamp_01 concrete_road_barrier steel_frame_shelves_01 utility_box_01 old_tyre metal_trash_can; do
    fetch_model "$model"
done

for surface in asphalt_02 concrete_floor brick_wall_08 metal_plate_02; do
    fetch_surface "$surface"
done

find "$destination" -type f -print0 | sort -z | xargs -0 sha256sum
