"""Explicit local maintenance transforms; never rewrite secrets or protocol preset names.

The caller owns backup, authenticated route-config commit, quiescence and restart.
This is not an automatic startup migration or a generic database replacement tool.
"""
import copy
import hashlib
import json

PROVIDER_IDS = {name: name + '-api' for name in (
    'nvidia', 'openrouter', 'you', 'exa', 'jina', 'websearchapi',
    'tavily', 'linkup', 'poe', 'groq', 'cohere')}
PROVIDER_IDS['chatgpt'] = 'codex-oauth'
EXPECTED_PRESETS = dict(zip(PROVIDER_IDS, (
    'nvidia-openai', 'openrouter-openai', 'you', 'exa', 'jina', 'websearchapi',
    'tavily', 'linkup', 'poe-openai', 'groq-openai', 'cohere-chat',
    'chatgpt-codex-oauth-official-api')))
ID_FIELDS = {'providerId', 'provider_id', 'providerAccountId'}
ID_LIST_FIELDS = {'providerIds', 'provider_ids', 'attemptedProviderIds'}


def rewrite_refs(value, mapping):
    """Only structured identity fields; arbitrary strings and credential IDs stay intact."""
    if isinstance(value, list):
        return [rewrite_refs(v, mapping) for v in value]
    if not isinstance(value, dict):
        return value
    result = {}
    for key, item in value.items():
        if key in ID_FIELDS and isinstance(item, str):
            result[key] = mapping.get(item, item)
        elif key in ID_LIST_FIELDS and isinstance(item, list):
            result[key] = [mapping.get(v, v) if isinstance(v, str) else v for v in item]
        else:
            result[key] = rewrite_refs(item, mapping)
    return result


def migrate_document(document):
    result = copy.deepcopy(document)
    existing = {p['id'] for p in document['providers']}
    mapping = {old: new for old, new in PROVIDER_IDS.items() if old in existing}
    if any(new in existing for new in mapping.values()):
        raise ValueError('Target provider ID already exists; merging is forbidden')
    for provider in result['providers']:
        old = provider['id']
        if old not in mapping:
            continue
        if provider.get('preset') != EXPECTED_PRESETS[old]:
            raise ValueError('Provider preset does not match the reviewed migration')
        if any(not c.get('id') for c in provider.get('credentials', [])):
            raise ValueError('Implicit credential IDs require a separate preservation plan')
        provider['id'] = mapping[old]
        provider['label'] = mapping[old]
        for plan in provider.get('test_plans', []):
            for scope in plan.get('scopes', []):
                if scope.get('kind') == 'account' and scope.get('id') == old + '::default':
                    scope['id'] = mapping[old] + '::default'
    # Routes are the provider-reference owner. Group member IDs are credential IDs.
    result['model_routes'] = rewrite_refs(result.get('model_routes', []), mapping)
    return result, mapping


def model_state_id(payload):
    credential = payload.get('providerCredentialId')
    if credential is None:
        credential = payload.get('providerCredentialRef') or ''
    parts = [payload['providerAccountId'], credential,
             payload.get('protocolProfile') or '', payload['model']]
    raw = b''.join(part.encode('utf-8') + b'\x1f' for part in parts)
    return 'pcms_' + hashlib.sha256(raw).hexdigest()


def encode(value):
    return json.dumps(value, ensure_ascii=False, separators=(',', ':'))


def migrate_runtime(db, mapping):
    """Atomic SQLite UPDATE only; caller must stop every writer before live execution.

    Nonempty queue/test/cash tables are deliberately rejected: their extra identity
    semantics are outside the current installation's reviewed, empty-table scope.
    """
    blocked = ('refill_tasks', 'refill_deliveries', 'credential_test_results',
               'credential_test_slots', 'gateway_cash_requests', 'local_balance_inflight')
    stats = {}
    with db:
        db.execute('BEGIN IMMEDIATE')
        for table in blocked:
            if db.execute(f'SELECT count(*) FROM "{table}"').fetchone()[0]:
                raise ValueError(f'{table} is not empty; migration requires additional review')
        for table in ('request_audits', 'credential_model_states', 'local_access_keys'):
            changed = 0
            rows = db.execute(f'SELECT id,payload FROM "{table}"')
            # Updating an identity while traversing its index can revisit the row.
            if table == 'credential_model_states':
                rows = rows.fetchall()
            for identity, raw in rows:
                before = json.loads(raw)
                after = copy.deepcopy(before)
                if table == 'request_audits':
                    old = after.get('providerAccountId')
                    if old in mapping:
                        after['providerAccountId'] = mapping[old]
                    if 'routeTrace' in after:
                        after['routeTrace'] = rewrite_refs(after['routeTrace'], mapping)
                elif table == 'local_access_keys':
                    if isinstance(after.get('metadata'), dict):
                        after['metadata'] = rewrite_refs(after['metadata'], mapping)
                else:
                    old = after.get('providerAccountId')
                    if old in mapping:
                        if identity != model_state_id(before) or before['id'] != identity:
                            raise ValueError('Model state identity is not canonical')
                        after['providerAccountId'] = mapping[old]
                        after['id'] = model_state_id(after)
                if after == before:
                    continue
                new_id = after['id'] if table == 'credential_model_states' else identity
                if new_id != identity and db.execute(
                        f'SELECT 1 FROM "{table}" WHERE id=?', (new_id,)).fetchone():
                    raise ValueError('Model state ID collision; merging is forbidden')
                db.execute(f'UPDATE "{table}" SET id=?,payload=? WHERE id=?',
                           (new_id, encode(after), identity))
                changed += 1
            stats[table] = changed
        for old, new in mapping.items():
            if db.execute('SELECT 1 FROM provider_pricing WHERE provider_id=?', (new,)).fetchone():
                raise ValueError('Provider pricing ID collision')
            cursor = db.execute('UPDATE provider_pricing SET provider_id=? WHERE provider_id=?', (new, old))
            stats['provider_pricing'] = stats.get('provider_pricing', 0) + cursor.rowcount
        if db.execute('PRAGMA integrity_check').fetchone()[0] != 'ok':
            raise ValueError('SQLite integrity check failed')
    return stats
