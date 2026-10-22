"""Offline integrity regression: native sources and executable build drivers are pinned."""
import importlib.util
import json
import io
import tarfile
import tempfile
import unittest
from pathlib import Path
from unittest.mock import patch

REPO = Path(__file__).resolve().parents[2]
SPEC = importlib.util.spec_from_file_location('cpa_prepare', REPO / 'tools/prepare-cpa-bssl.py')
PREPARE = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(PREPARE)


class PreparationIntegrityTests(unittest.TestCase):
    def test_archive_rejects_traversal_and_links(self):
        local = REPO.parent.parent / 'linshi'
        with tempfile.TemporaryDirectory(prefix='cpa-extraction-', dir=local) as directory:
            for name, kind in [('../escape', tarfile.REGTYPE), ('link', tarfile.SYMTYPE),
                               ('device', tarfile.CHRTYPE)]:
                data = io.BytesIO()
                with tarfile.open(fileobj=data, mode='w') as archive:
                    member = tarfile.TarInfo(name)
                    member.type = kind
                    member.linkname = '../escape'
                    archive.addfile(member)
                data.seek(0)
                with tarfile.open(fileobj=data, mode='r') as archive:
                    with self.assertRaisesRegex(RuntimeError, 'unsafe|not supported'):
                        PREPARE.extract_pinned_archive(archive, Path(directory))

    def test_build_driver_is_verified_on_reuse(self):
        local = REPO.parent.parent / 'linshi'
        with tempfile.TemporaryDirectory(prefix='cpa-integrity-', dir=local if local.is_dir() else None) as directory:
            root = Path(directory)
            profile = root / 'profile'
            (profile / 'compat').mkdir(parents=True)
            archive = root / 'archive.crate'
            archive.write_bytes(b'isolated archive fixture')
            output = root / 'prepared'
            package = output / 'rama-boring-sys-0.6.20'
            driver = package / 'build/main.rs'
            driver.parent.mkdir(parents=True)
            driver.write_text('fn main() {}', encoding='utf-8')
            source = package / 'deps/boringssl'
            source.mkdir(parents=True)
            (source / 'source.cc').write_text('int fixture;', encoding='utf-8')
            files = PREPARE.prepared_files(package, output)
            key = 'rama-boring-sys-0.6.20/build/main.rs'
            self.assertIn(key, files)
            receipt = {'archive_sha256': PREPARE.digest(archive),
                       'inputs': {Path(PREPARE.__file__).name: PREPARE.digest(Path(PREPARE.__file__))},
                       'files': files}
            (output / 'receipt.json').write_text(json.dumps(receipt), encoding='utf-8')
            with patch.object(PREPARE, 'ARCHIVE', archive), patch.object(PREPARE, 'PROFILE', profile), \
                    patch.object(PREPARE, 'ARCHIVE_SHA', PREPARE.digest(archive)):
                self.assertEqual(PREPARE.prepare(output), source)
                driver.write_text('fn main() { changed(); }', encoding='utf-8')
                with self.assertRaisesRegex(RuntimeError, 'prepared source modified'):
                    PREPARE.prepare(output)


if __name__ == '__main__':
    unittest.main()
