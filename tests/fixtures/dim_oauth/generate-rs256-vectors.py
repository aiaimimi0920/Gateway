"""Generate synthetic RS256 vectors once; no account or production key material is used."""
import base64
import hashlib
import json
from pathlib import Path
from cryptography.hazmat.primitives import hashes
from cryptography.hazmat.primitives.asymmetric import padding, rsa

def encoded(value):
    return base64.urlsafe_b64encode(value).decode().rstrip('=')

def wire(value):
    return encoded(json.dumps(value, separators=(',', ':')).encode())

key = rsa.generate_private_key(public_exponent=65537, key_size=2048)
public = key.public_key().public_numbers()
jwk = {'kty': 'RSA', 'kid': 'synthetic-dim-rsa', 'alg': 'RS256', 'use': 'sig',
       'key_ops': ['verify'], 'n': encoded(public.n.to_bytes(256, 'big')),
       'e': encoded(public.e.to_bytes(3, 'big'))}
claims = {'iss': 'https://dimagent.cn', 'sub': 'synthetic-subject',
          'aud': 'permitted-gateway', 'nonce': 'synthetic-nonce',
          'iat': 1800000000, 'exp': 1800003600,
          'at_hash': encoded(hashlib.sha256(b'synthetic-access').digest()[:16])}
header = {'alg': 'RS256', 'kid': jwk['kid'], 'typ': 'JWT'}

def signed(payload, jose=None):
    message = wire(header if jose is None else jose) + '.' + wire(payload)
    signature = key.sign(message.encode(), padding.PKCS1v15(), hashes.SHA256())
    return message + '.' + encoded(signature)

vectors = {'now': 1800000100, 'nonce': 'synthetic-nonce', 'access': 'synthetic-access',
           'jwks': {'keys': [jwk]}, 'valid': signed(claims), 'rejected': []}
without_nonce = {k: v for k, v in claims.items() if k != 'nonce'}
vectors['refresh_without_nonce'] = signed(without_nonce)
changes = [
    ('iss', 'https://dimagent.com'), ('sub', ''), ('aud', 'other-client'),
    ('aud', ['permitted-gateway', 'other-client']), ('azp', 'other-client'),
    ('nonce', 'other-nonce'), ('exp', 1800000001), ('iat', 1800000300),
    ('nbf', 1800000300), ('at_hash', 'wrong-hash'),
]
for field, value in changes:
    changed = dict(claims)
    changed[field] = value
    vectors['rejected'].append({'reason': field, 'token': signed(changed)})
for field, value in [('alg', 'HS256'), ('jku', 'https://evil.invalid/key'), ('crit', ['custom'])]:
    changed = dict(header)
    changed[field] = value
    vectors['rejected'].append({'reason': field, 'token': signed(claims, changed)})
target = Path(__file__).with_name('rs256-vectors.json')
target.write_text(json.dumps(vectors, indent=2) + '\n', encoding='utf-8')
print(f'{target}: synthetic public keys and signed tokens only; private key discarded')
