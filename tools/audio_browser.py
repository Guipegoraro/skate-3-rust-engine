"""Local sound browser for picking event sounds (SK-022/023).

Writes <assets>/private/audio/index.html next to the decoded sounds: one row per sound with
an audio player, its id ("<bank>/<index>"), length and loop points. Open it in a browser,
listen, and copy the ids into crates/skate-game/src/audio/events.json. The page and the
sounds stay in the user's data folder; nothing here is committed or published.

Usage: python tools/audio_browser.py [assets-dir]   (default: ./assets)
"""
import html
import json
import sys
from pathlib import Path


def main():
    assets = Path(sys.argv[1] if len(sys.argv) > 1 else 'assets')
    audio = assets/'private/audio'
    sounds = json.loads((audio/'audio.json').read_text(encoding='utf-8'))['sounds']
    rows = []
    for s in sounds:
        seconds = (s['samples'] or 0) / (s['rate'] or 1)
        loop = f"{s['loop_start']}..{s['loop_end']}" if s.get('loop_end') else ''
        relative = Path(s['file']).relative_to('private/audio').as_posix()
        rows.append(f"<tr data-bank='{html.escape(s['bank'])}'><td><code>{html.escape(s['id'])}</code></td>"
                    f"<td>{seconds:.2f}s</td><td>{loop}</td>"
                    f"<td><audio controls preload='none' src='{html.escape(relative)}'></audio></td></tr>")
    banks = sorted({s['bank'] for s in sounds})
    options = ''.join(f"<option>{html.escape(b)}</option>" for b in banks)
    page = f"""<!doctype html><meta charset=utf-8><title>Skate 3 sounds</title>
<style>body{{font:14px system-ui;margin:16px}}td{{padding:2px 8px}}code{{user-select:all}}</style>
<h1>Skate 3 sounds ({len(sounds)})</h1>
<p>Click an id to select it, then paste it into events.json.
<select id=bank onchange="for(const r of document.querySelectorAll('tr[data-bank]'))r.hidden=this.value&&r.dataset.bank!=this.value">
<option value=''>All banks</option>{options}</select></p>
<table>{''.join(rows)}</table>"""
    (audio/'index.html').write_text(page, encoding='utf-8')
    print(audio/'index.html')


if __name__ == '__main__':
    main()
