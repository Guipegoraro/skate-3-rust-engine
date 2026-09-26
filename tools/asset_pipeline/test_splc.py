import struct
import unittest

from tools.asset_pipeline.splc import split


def snr(samples, payload):
    # EAAC SNR: version 0, codec 3 (EA-XMA), mono, 22050 Hz; RAM type; then the data.
    return struct.pack('>II', (3 << 24) | 22050, samples) + payload


class Splc(unittest.TestCase):
    def test_splits_samples_by_table_offsets(self):
        first, second, third = snr(100, b'a' * 12), snr(200, b'b' * 6), snr(300, b'c' * 9)
        blob = first + second + third
        rows = b''.join(struct.pack('>III', start - 0x14, h, start)
                        for start, h in ((len(first), 0x22), (len(first) + len(second), 0x33)))
        header = b'SPLC' + struct.pack('>IIII', 3, 0x20, 1, 3) + b'\0' * 12
        data = header + rows + struct.pack('>II', len(blob), 0x11) + blob
        samples = split(data)
        self.assertEqual([h for h, _ in samples], [0x11, 0x22, 0x33])
        self.assertEqual([s for _, s in samples], [first, second, third])

    def test_rejects_other_files(self):
        with self.assertRaises(ValueError):
            split(b'ABKC' + b'\0' * 64)


if __name__ == '__main__':
    unittest.main()
