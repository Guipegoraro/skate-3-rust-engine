"""Export selected original front-end textures for engine HUD overlays.

Add a UI bundle by adding one entry to BUNDLES. Output (private, never committed):
assets/private/hud/ui/<key>/<texture>.rgba plus assets/private/hud/ui/index.json mapping
"<key>/<texture>" to {file, width, height}. The engine reads it through ui_textures.rs.
"""
import argparse
import json
import shutil
from pathlib import Path

from vendor.skate3_ui.project import extract_project

# key -> archive path prefix inside the FE .big archives.
BUNDLES = {
    'hud2/trickanalyser': 'data/fe/source/screens/hud2/trickanalyser',
    'controls/gesture_item': 'data/fe/source/controls/gesture_item',
}


def texture_key(name):
    # "trickanalyser_textures\\1.Texture" -> "1"
    return name.replace('\\', '/').rsplit('/', 1)[-1].removesuffix('.Texture')


def export(game, assets, work):
    extract_project(game, work, prefixes=tuple(BUNDLES.values()), update=True)
    output = assets/'private/hud/ui'
    if output.exists():
        shutil.rmtree(output)
    index = {}
    for key, prefix in BUNDLES.items():
        bundle = work/'assets'/prefix
        manifest = json.loads((bundle/'manifest.json').read_text(encoding='utf-8'))
        for texture in manifest['textures']:
            source = bundle/texture['rgba_file']
            if source.stat().st_size != texture['width']*texture['height']*4:
                raise ValueError(f'Unexpected RGBA size for {prefix}/{texture["name"]}')
            name = texture_key(texture['name'])
            target = output/key/f'{name}.rgba'
            target.parent.mkdir(parents=True, exist_ok=True)
            shutil.copyfile(source, target)
            index[f'{key}/{name}'] = {'file': f'{key}/{name}.rgba',
                                     'width': texture['width'], 'height': texture['height']}
    (output/'index.json').write_text(json.dumps(index, indent=1, sort_keys=True), encoding='utf-8')
    print(f'Original UI textures ready: {len(index)}', flush=True)
    return index


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--game', type=Path, required=True)
    parser.add_argument('--assets', type=Path, required=True)
    parser.add_argument('--work', type=Path, required=True)
    args = parser.parse_args()
    export(args.game, args.assets, args.work)
