"""Match a Skate3Recomp XMA trace to our decoded sound ids (SK-032).

The patched ReXGlue runtime (REX_XMA_TRACE=<file>) logs one line per new XMA input buffer:
"<ms> <context> <size> <first 32 bytes hex>". The game's sample data is byte-identical in
memory and on disc, so each prefix is searched in the audiofiles.big banks: SPLC (.bnk)
hits resolve to "<bank>/<index>" like audio.json; ABK hits resolve to the bank.

Usage: python tools/audio_trace_match.py <trace.log> <game root>
"""
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
from tools.owned_game.big import BigArchive  # noqa: E402
from tools.asset_pipeline.splc import split  # noqa: E402


def load_banks(game_root):
    archive = BigArchive(Path(game_root)/'data/audio/audiofiles.big')
    banks = []
    for entry in archive.entries:
        path = Path(entry.path)
        suffix = path.suffix.lower()
        if suffix not in ('.bnk', '.abk'):
            continue
        data = archive.read(entry)
        slices = []
        if suffix == '.bnk':
            try:
                offset = 0
                for index, (_, snr) in enumerate(split(data), start=1):
                    start = data.find(snr[:64], offset)
                    slices.append((start, start + len(snr), index))
                    offset = start + 1
            except ValueError:
                pass
        banks.append((path.stem, data, slices))
    return banks


def locate(banks, prefix):
    for name, data, slices in banks:
        at = data.find(prefix)
        if at < 0:
            continue
        for start, end, index in slices:
            if start <= at < end:
                return f'{name}/{index}'
        return f'{name} (offset {at:#x})'
    return None


def main():
    trace, game_root = sys.argv[1], sys.argv[2]
    banks = load_banks(game_root)
    seen = {}
    for line in Path(trace).read_text().splitlines():
        parts = line.split()
        if len(parts) < 4:
            continue
        ms, context, size, head = int(parts[0]), parts[1], int(parts[2]), bytes.fromhex(parts[3])
        key = head
        if key not in seen:
            seen[key] = locate(banks, head) or 'unknown (streamed or not in audiofiles.big)'
        print(f'{ms / 1000:9.3f}s ctx {context:>3} {size:7d} B  {seen[key]}')


if __name__ == '__main__':
    main()
