"""Retention policy shared by every project-owned measurement history writer."""
import os

ENVIRONMENT_KEY = 'DCTX0_PRESERVE_BENCHMARK_HISTORY'


def preserve_history(explicit=False):
    return bool(explicit) or os.environ.get(ENVIRONMENT_KEY) == '1'


def mark(record, explicit=False):
    record['history_retention'] = 'preserve-all' if preserve_history(explicit) else 'latest-20'
    return record


def prune(history, explicit=False):
    """Call while holding the writer's existing history lock."""
    if not preserve_history(explicit):
        for old in sorted(history.glob('*.json'))[:-20]:
            old.unlink()
