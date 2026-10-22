"""Prepare pinned, isolated BoringSSL sources. Never edit Cargo's registry or delete a tree."""
import argparse
import hashlib
import json
import shutil
import subprocess
import tarfile
from pathlib import Path

REPO = Path(__file__).resolve().parents[1]
PROFILE = REPO / 'crates/cpa-tls-profile'
ARCHIVE = PROFILE / 'upstream/rama-boring-sys-0.6.20.crate'
ARCHIVE_SHA = 'ea17f614cc3eff7c149e8a68a4645fb6f556a31b768f6bbcb4162e6d42a2bba7'

def digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()

def prepared_files(package, output):
    return {str(p.relative_to(output)).replace('\\', '/'): digest(p)
            for p in package.rglob('*') if p.is_file() and '.git' not in p.parts}

def replace(root, name, old, new):
    path = root / name
    text = path.read_text(encoding='utf-8')
    if text.count(old) != 1:
        raise RuntimeError(f'pinned patch drift: {name}')
    path.write_text(text.replace(old, new), encoding='utf-8', newline='\n')

def run(args, cwd):
    subprocess.run(args, cwd=cwd, check=True)

def extract_pinned_archive(archive, output):
    # Python 3.11-compatible safe extraction. The hash-bound archive needs no links/devices.
    root = output.resolve()
    for member in archive.getmembers():
        target = (root / member.name).resolve()
        if not target.is_relative_to(root) or member.name.startswith(('/', '\\')):
            raise RuntimeError('unsafe archive member path')
        if member.isdir():
            target.mkdir(parents=True, exist_ok=True)
        elif member.isfile():
            target.parent.mkdir(parents=True, exist_ok=True)
            with archive.extractfile(member) as source, target.open('wb') as destination:
                shutil.copyfileobj(source, destination)
            target.chmod(member.mode & 0o777)
        else:
            raise RuntimeError('archive links and special files are not supported')

def prepare(output):
    if digest(ARCHIVE) != ARCHIVE_SHA:
        raise RuntimeError('upstream archive digest mismatch')
    inputs = {p.name: digest(p) for p in [Path(__file__), *sorted((PROFILE/'compat').glob('*'))] if p.is_file()}
    receipt = output / 'receipt.json'
    if receipt.exists():
        saved = json.loads(receipt.read_text(encoding='utf-8'))
        if saved['archive_sha256'] != ARCHIVE_SHA or saved['inputs'] != inputs:
            raise RuntimeError('source preparation changed; choose a new output directory')
        if any(not (output/name).is_file() or digest(output/name) != sha for name, sha in saved['files'].items()):
            raise RuntimeError('prepared source modified; choose a new output directory')
        return output / 'rama-boring-sys-0.6.20/deps/boringssl'
    if output.exists() and any(output.iterdir()):
        raise RuntimeError('output must be empty; existing contents are preserved')
    output.mkdir(parents=True, exist_ok=True)
    with tarfile.open(ARCHIVE, 'r:gz') as archive:
        extract_pinned_archive(archive, output)
    package = output / 'rama-boring-sys-0.6.20'
    source = package / 'deps/boringssl'
    run(['git', 'init', '--quiet'], source)
    for patch in ['rama_tls.patch', 'rama_boring_pq.patch', 'rama_symbols.patch',
                  'rama_boringssl_cmake.patch', 'rama_boring_prefix.patch']:
        run(['git', 'apply', '--whitespace=fix', str(package/'patches'/patch)], source)
    replace(source, 'include/openssl/ssl.h', '#define SSL_GROUP_MLKEM1024 0x0202',
            '#define SSL_GROUP_MLKEM1024 0x0202\n#define SSL_GROUP_SECP256R1_MLKEM768 0x11eb\n#define SSL_GROUP_SECP384R1_MLKEM1024 0x11ed')
    replace(source, 'ssl/internal.h', 'constexpr size_t kNumNamedGroups = 10u;', 'constexpr size_t kNumNamedGroups = 12u;')
    replace(source, 'ssl/extensions.cc', '    case SSL_GROUP_MLKEM1024:\n      return true;',
            '    case SSL_GROUP_MLKEM1024:\n    case SSL_GROUP_SECP256R1_MLKEM768:\n    case SSL_GROUP_SECP384R1_MLKEM1024:\n      return true;')
    replace(source, 'crypto/obj/objects.txt', ' : X25519MLKEM768\n',
            ' : X25519MLKEM768\n : SecP256r1MLKEM768\n : SecP384r1MLKEM1024\n')
    # Generated once by the pinned upstream objects.go; checked-in delta avoids a Go build dependency.
    run(['git', 'apply', '--check', str(PROFILE/'compat/objects.patch')], source)
    run(['git', 'apply', str(PROFILE/'compat/objects.patch')], source)
    # Match the pinned Go generator bytes, independently of Git's host newline policy.
    for name in ['include/openssl/nid.h', 'crypto/obj/obj_dat.h', 'crypto/obj/obj_mac.num']:
        path = source / name
        path.write_text(path.read_text(encoding='utf-8'), encoding='utf-8', newline='\n')
    shutil.copyfile(PROFILE/'compat/hybrid_key_share.cc', source/'ssl/gateway_hybrid_key_share.cc')
    replace(source, 'ssl/ssl_key_share.cc', 'constexpr NamedGroup kNamedGroups[] = {',
            '#include "gateway_hybrid_key_share.cc"\n\nconstexpr NamedGroup kNamedGroups[] = {\n'
            '    {NID_SecP256r1MLKEM768, SSL_GROUP_SECP256R1_MLKEM768, "SecP256r1MLKEM768", ""},\n'
            '    {NID_SecP384r1MLKEM1024, SSL_GROUP_SECP384R1_MLKEM1024, "SecP384r1MLKEM1024", ""},')
    replace(source, 'ssl/ssl_key_share.cc', '  switch (group_id) {',
            '  switch (group_id) {\n    case SSL_GROUP_SECP256R1_MLKEM768:\n      return MakeUnique<GatewayECHybridKeyShare>(false);\n'
            '    case SSL_GROUP_SECP384R1_MLKEM1024:\n      return MakeUnique<GatewayECHybridKeyShare>(true);')
    # The host driver executes package/build/*.rs, not only native sources.
    # Include every extracted input so a reused tree cannot silently change it.
    files = prepared_files(package, output)
    receipt.write_text(json.dumps({'archive_sha256': ARCHIVE_SHA, 'inputs': inputs, 'files': files}, indent=2), encoding='utf-8')
    return source

if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--output', type=Path, required=True)
    args = parser.parse_args()
    print(prepare(args.output.resolve()))
