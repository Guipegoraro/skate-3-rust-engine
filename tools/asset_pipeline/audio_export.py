"""Owned-disc audio (SK-020): decode selected EA sound banks with vgmstream-cli.

Writes assets/private/audio/<bank>/<index>.wav plus assets/private/audio/audio.json:
{"version": 1, "sounds": [{id, file, bank, index, rate, channels, samples, loop_start, loop_end}]}.
The id is "<bank>/<index>" (index = vgmstream subsong, 1-based). Only the banks the engine
uses are decoded; the full disc would be over 1 GB of PCM.
"""
import json
import subprocess
from pathlib import Path

from tools.owned_game.big import BigArchive

# audiofiles.big sound banks (.abk) used by the skate one-shots and loops (SK-022/023).
BANKS = ('GRINDS', 'board_scrapes', 'Brd_Squeaks', 'WHEEL_SKID_BANK', 'Bodyslide',
         'Sk8_Air_Flip_Tricks', 'Seams_Bank', 'Rolling_Rattles', 'sense_of_speed',
         'fstep_skateshoe1_sm', 'FOOT_DRAG')


# audiofiles.big SPLC banks (.bnk, see splc.py): board collisions/pops/landings, metal hits,
# clothing foley, menu sounds and whooshes.
SPLC_BANKS = ('Skate_Collisions', 'Skate_Metal', 'sk8_foley', 'sk8_menu', 'Sk82_Whsh_Bys')


def _vgmstream(tool, *args):
    kwargs = {'creationflags': subprocess.CREATE_NO_WINDOW} if hasattr(subprocess, 'CREATE_NO_WINDOW') else {}
    done = subprocess.run([str(tool), *map(str, args)], capture_output=True, text=True, **kwargs)
    if done.returncode != 0:
        raise RuntimeError(f'vgmstream failed on {args[-1]}: {done.stdout[-400:]}{done.stderr[-400:]}')
    return done.stdout


def _info(tool, source):
    """One JSON object per subsong from `-m -I`."""
    first = json.loads(_vgmstream(tool, '-m', '-I', source).strip().splitlines()[-1])
    count = (first.get('streamInfo') or {}).get('total') or 1
    infos = [first]
    for index in range(2, count + 1):
        infos.append(json.loads(_vgmstream(tool, '-m', '-I', '-s', index, source).strip().splitlines()[-1]))
    return infos


def _record(bank, index, info):
    loop = info.get('loopingInfo') or {}
    return {
        'id': f'{bank}/{index}', 'file': f'private/audio/{bank}/{index}.wav',
        'bank': bank, 'index': index,
        'rate': info.get('sampleRate'), 'channels': info.get('channels'),
        'samples': info.get('numberOfSamples'),
        'loop_start': loop.get('start') if loop else None,
        'loop_end': loop.get('end') if loop else None,
    }


def _export_splc(bank, data, audio, work, tool):
    """Each SPLC sample is a standalone EA SNR; decode them one by one (index from 1)."""
    from .splc import split
    target = audio/bank
    target.mkdir(exist_ok=True)
    sounds = []
    for index, (_, snr) in enumerate(split(data), start=1):
        source = work/f'{bank}-{index}.snr'
        source.write_bytes(snr)
        _vgmstream(tool, '-i', '-o', target/f'{index}.wav', source)
        info = json.loads(_vgmstream(tool, '-m', '-I', source).strip().splitlines()[-1])
        sounds.append(_record(bank, index, info))
        source.unlink()
    return sounds


def export(game_root, private, work, tool, report=print):
    audio = private/'audio'
    audio.mkdir(parents=True, exist_ok=True)
    work.mkdir(parents=True, exist_ok=True)
    wanted = {name.lower() for name in BANKS}
    wanted_splc = {name.lower() for name in SPLC_BANKS}
    archive = BigArchive(Path(game_root)/'data/audio/audiofiles.big')
    sounds = []
    for entry in archive.entries:
        path = Path(entry.path)
        if path.suffix.lower() == '.bnk' and path.stem.lower() in wanted_splc:
            report(f'Decoding sound bank {path.stem}')
            sounds.extend(_export_splc(path.stem, archive.read(entry), audio, work, tool))
            continue
        if path.suffix.lower() != '.abk' or path.stem.lower() not in wanted:
            continue
        bank = path.stem
        report(f'Decoding sound bank {bank}')
        source = work/path.name
        source.write_bytes(archive.read(entry))
        target = audio/bank
        target.mkdir(exist_ok=True)
        _vgmstream(tool, '-i', '-s', 1, '-S', 0, '-o', target/'?s.wav', source)
        for index, info in enumerate(_info(tool, source), start=1):
            file = target/f'{index}.wav'
            if not file.is_file():
                continue
            sounds.append(_record(bank, index, info))
        source.unlink()
    missing = (wanted | wanted_splc) - {s['bank'].lower() for s in sounds}
    (audio/'audio.json').write_text(json.dumps({'version': 1, 'sounds': sounds,
                                                'missing_banks': sorted(missing)}), encoding='utf-8')
    return sounds
