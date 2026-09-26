"""EA "SPLC" sound banks (Skate 3 `audiofiles.big/*.bnk`), reverse engineered for SK-022.

Layout (big endian), as observed in sk8_foley.bnk and Skate_Collisions.bnk:
  0x00 'SPLC', 0x04 version (3), 0x08 offset, 0x0C patch count, 0x10 sample count, 0x1C name.
  Patch records (0x24 bytes: index, group, float parameters) follow, then more tables.
  Sample table: one (u32 previous_end, u32 name_hash, u32 header_offset) per sample after the
  first, then u32 blob size and u32 name hash of sample 0; offsets count from after that hash.
  Each sample in the blob is an EA SNR header (EAAC, codec 3 = EA-XMA, RAM type) followed by
  its data, so each [header_offset, next header_offset) slice is a standalone .snr that
  vgmstream decodes. The first sample sits at offset 0 and has no table row.
"""
import struct

RATES = {11025, 16000, 22050, 24000, 32000, 44100, 48000}


def _snr(data, offset):
    if offset + 8 > len(data):
        return False
    w0, w1 = struct.unpack_from('>II', data, offset)
    return w0 >> 28 == 0 and (w0 >> 24) & 0xF in (2, 3, 4, 5, 6, 7) and (w0 & 0x3FFFF) in RATES \
        and w1 >> 30 in (0, 1) and 0 < (w1 & 0x1FFFFFFF) < 20_000_000


def _row(data, offset):
    previous_end, name_hash, header = struct.unpack_from('>III', data, offset)
    return previous_end, name_hash, header


def split(data):
    """[(name_hash or None, snr_bytes)] for every sample in an SPLC bank."""
    if data[:4] != b'SPLC':
        raise ValueError('not an SPLC bank')
    # Before the blob: u32 blob size, u32 name hash of sample 0. Offsets count from after the hash.
    for base in range(struct.unpack_from('>I', data, 8)[0], len(data) - 8, 4):
        size = struct.unpack_from('>I', data, base - 8)[0]
        if _snr(data, base) and 0 <= len(data) - (base - 4 + size) < 0x800:
            break
    else:
        raise ValueError('SPLC sample blob not found')
    first_hash = struct.unpack_from('>I', data, base - 4)[0]
    rows = []
    offset = base - 8 - 12
    while offset >= 0:
        previous_end, name_hash, header = _row(data, offset)
        if not (0 < header - previous_end < 0x80 and _snr(data, base + header)):
            break
        rows.append((name_hash, header))
        offset -= 12
    rows.reverse()
    starts = [(first_hash, 0)] + rows
    samples = []
    for index, (name_hash, start) in enumerate(starts):
        end = starts[index + 1][1] if index + 1 < len(starts) else min(size, len(data) - base)
        samples.append((name_hash, data[base + start:base + end]))
    return samples
