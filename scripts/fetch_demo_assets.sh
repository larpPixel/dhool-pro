#!/usr/bin/env sh
set -eu

root=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
destination="$root/assets/demo/suzanne"
base="https://raw.githubusercontent.com/KhronosGroup/glTF-Sample-Assets/main/Models/Suzanne/glTF"

mkdir -p "$destination"
for file in Suzanne.gltf Suzanne.bin Suzanne_BaseColor.png Suzanne_MetallicRoughness.png; do
    curl --fail --location --silent --show-error "$base/$file" -o "$destination/$file"
done

sha256sum "$destination"/*
