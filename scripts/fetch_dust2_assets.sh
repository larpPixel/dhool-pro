#!/usr/bin/env sh
set -eu

# Legacy fetcher for the old CC0 desert placeholder bundle in assets/dust2/.
# The current --map dust2 path uses scripts/import_cs2_dust2.sh and a local,
# git-ignored CS2 export instead of these Poly Haven placeholders.

root=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
destination="$root/assets/dust2"
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

# Desert ground / sand surfaces.
for surface in sand_01 coast_sand_05; do
    fetch_surface "$surface"
done

# Masonry and metal details for walls, doors, and props.
for surface in yellow_stone_wall rusty_metal; do
    fetch_surface "$surface"
done

# Common dust2-style props: crates and barrels.
for model in wooden_crate_02 barrel_03; do
    fetch_model "$model"
done

find "$destination" -type f -print0 | sort -z | xargs -0 sha256sum
