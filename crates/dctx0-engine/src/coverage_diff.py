"""Read-only Git adapter. Ranges always refer to the resolved base revision."""
import re


def coverage_diff(options):
    root = Path(options['root'])
    env = dict(os.environ)
    def git(*args):
        return run(['git', '--literal-pathspecs', *args], root, env, 30)[0]
    revision = git('rev-parse', '--verify', '--end-of-options', options['base'] + '^{commit}').decode().strip()
    prefix = ['diff', '--relative', '--no-ext-diff', '--no-textconv', '--no-renames', '--no-color']
    def patch():
        return git(*prefix, '--unified=0', revision, '--', '.')
    before = patch()
    names = git(*prefix, '--name-status', '-z', revision, '--', '.').decode().split('\0')
    files = []
    for index in range(0, len(names) - 1, 2):
        status, name = names[index:index + 2]
        contents = git(*prefix, '--unified=0', revision, '--', './' + name).decode()
        ranges = []
        for match in re.finditer(r'^@@ -(\d+)(?:,(\d+))? \+(\d+)(?:,(\d+))? @@', contents, re.MULTILINE):
            old, old_count, new, new_count = (int(v) if v is not None else 1 for v in match.groups())
            ranges.append({'old_start': old, 'old_count': old_count, 'new_start': new, 'new_count': new_count})
        files.append({'path': name, 'status': status, 'binary': 'Binary files ' in contents or 'GIT binary patch' in contents,
                      'ranges': ranges})
    for name in git('ls-files', '--others', '--exclude-standard', '-z', '--', '.').decode().split('\0'):
        if name:
            files.append({'path': name, 'status': 'untracked', 'binary': False, 'ranges': []})
    if patch() != before:
        raise RuntimeError('Working-tree diff changed during inspection; retry against a stable tree')
    return {'base_revision': revision, 'files': sorted(files, key=lambda file: file['path'])}
