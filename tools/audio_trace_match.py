"""Match a Skate3Recomp XMA trace to our decoded sound ids (SK-032).

The patched ReXGlue runtime (REX_XMA_TRACE=<file>) logs one line per new XMA input buffer:
"<ms> <context> <size> <first 32 bytes hex>". The game's sample data is byte-identical in
memory and on disc, so each prefix is searched in the audiofiles.big banks: SPLC (.bnk)
hits resolve to "<bank>/<index>" like audio.json. ABK samples are inline EA SNR headers
(03 <channels> <rate> <flags|samples>) followed by their data; each decoded subsong is found by
its sample count from assets/private/audio/audio.json, so ABK hits resolve to "<bank>/<index>"
too (banks we do not decode stay "<bank> (offset ...)").

Lines "# <ms> <text>" are markers written by the scripted pad (REX_PAD_SCRIPT) and are printed
as they are, so each action's sounds follow its marker.

Usage: python tools/audio_trace_match.py <trace.log> <game root> [--known] [--from <ms>]
       [--recurring <label> <seconds>]
  --known      hide buffers that are not in the banks (streams: music, ambience, video)
  --from       skip lines before this time (ms)
  --recurring  for markers "<label> 1", "<label> 2"...: the ids that start within <seconds> after
               most of them, i.e. the action's sounds rather than the living world around it
"""
import argparse
import json
import re
import struct
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
from tools.owned_game.big import BigArchive  # noqa: E402
from tools.asset_pipeline.splc import split  # noqa: E402


def abk_slices(data, subsongs):
    """(start, end, index) per decoded subsong, from its SNR header located by sample count."""
    headers = {}
    for match in re.finditer(rb'\x03[\x00-\x1c](?=.{6})', data, re.DOTALL):
        at = match.start()
        rate, count = struct.unpack('>HI', data[at + 2:at + 8])
        headers.setdefault((rate, count & 0x0FFFFFFF), []).append(at)
    starts = []
    for sound in subsongs:
        offsets = headers.get((sound['rate'], sound['samples']))
        if offsets:
            starts.append((offsets.pop(0), sound['index']))
    starts.sort()
    ends = [s for s, _ in starts[1:]] + [len(data)]
    return [(start, end, index) for (start, index), end in zip(starts, ends)]


def load_banks(game_root, audio_json=None):
    decoded = {}
    audio_json = Path(audio_json or Path(__file__).resolve().parents[1]/'assets/private/audio/audio.json')
    if audio_json.is_file():
        for sound in json.loads(audio_json.read_text(encoding='utf-8'))['sounds']:
            decoded.setdefault(sound['bank'], []).append(sound)
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
        elif path.stem in decoded:
            slices = abk_slices(data, decoded[path.stem])
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


def matched(trace, banks, start):
    """(ms, marker text or None, context, id or None) per trace line from `start` ms."""
    seen = {}
    for line in Path(trace).read_text().splitlines():
        parts = line.split()
        if line.startswith('#'):
            if len(parts) >= 2 and int(parts[1]) >= start:
                yield int(parts[1]), ' '.join(parts[2:]), None, None
            continue
        if len(parts) < 4 or int(parts[0]) < start:
            continue
        head = bytes.fromhex(parts[3])
        if head not in seen:
            seen[head] = locate(banks, head)
        yield int(parts[0]), None, parts[1], seen[head]


def recurring(rows, label, seconds):
    """Ids that start within `seconds` after most '<label> <n>' markers, with their delays."""
    pattern = re.compile(re.escape(label) + r' \d+')
    marks = [ms for ms, text, _, _ in rows if text and pattern.fullmatch(text)]
    # A sound starts when its context was idle or played something else: a loop's next buffers
    # (wind, rolling, ambience) do not count as new sounds.
    starts, last = [], {}
    for ms, _, context, sound in rows:
        if context is None:
            continue
        sound = sound and sound.split(' (offset')[0]
        previous = last.get(context)
        if sound and (previous is None or previous[1] != sound or ms - previous[0] > 1000):
            starts.append((ms, sound))
        last[context] = (ms, sound)
    delays = {}
    for mark in marks:
        first = {}
        for ms, sound in starts:
            if mark <= ms < mark + seconds * 1000:
                first.setdefault(sound, ms - mark)
        for sound, delay in first.items():
            delays.setdefault(sound, []).append(delay / 1000)
    print(f'{len(marks)} "{label}" markers')
    for sound, found in sorted(delays.items(), key=lambda kv: (-len(kv[1]), min(kv[1]))):
        if len(found) >= max(2, len(marks) // 2):
            print(f'{len(found)}/{len(marks)} {sound:30s} after {", ".join(f"{d:.2f}" for d in sorted(found))} s')


def main():
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument('trace')
    parser.add_argument('game_root')
    parser.add_argument('--known', action='store_true', help='hide buffers not found in the banks')
    parser.add_argument('--from', dest='start', type=int, default=0, help='skip lines before this ms')
    parser.add_argument('--recurring', nargs=2, metavar=('LABEL', 'SECONDS'),
                        help='summarise ids that follow most "<LABEL> <n>" markers')
    args = parser.parse_args()
    rows = list(matched(args.trace, load_banks(args.game_root), args.start))
    if args.recurring:
        recurring(rows, args.recurring[0], float(args.recurring[1]))
        return
    for ms, text, context, sound in rows:
        if text is not None:
            print(f'{ms / 1000:9.3f}s --- {text}')
        elif sound or not args.known:
            print(f'{ms / 1000:9.3f}s ctx {context:>3}  {sound or "unknown (streamed or not in audiofiles.big)"}')


if __name__ == '__main__':
    main()
