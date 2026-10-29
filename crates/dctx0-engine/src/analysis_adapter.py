def state_paths(options, prefix):
    original = Path(options.get('project_root', options['root'])).resolve()
    for _ in Path(prefix).parts:
        original = original.parent
    try:
        relative = Path(options['database']).resolve().relative_to(original)
    except (KeyError, ValueError):
        return set()
    return {str(relative) + suffix for suffix in ('', '-wal', '-shm', '-journal', '.lock')}


"""Git-visible snapshot adapter. Process execution is provided by coverage_collect."""

def analysis_adapter(options):
    root = Path(options['root']).resolve()
    timeout = options.get('timeout_secs', 300)
    def git(*args):
        return run(['git', '--literal-pathspecs', *args], root, dict(os.environ), timeout)[0]
    action = options['action']
    if action == 'resolve':
        return git('rev-parse', '--verify', '--end-of-options', options['revision'] + '^{commit}').decode().strip()
    if action == 'ancestor':
        return run(['git', 'merge-base', '--is-ancestor', options['base'], options['head']], root, dict(os.environ), timeout, allow_failure=True)[2] == 0
    if action == 'snapshot':
        try:
            repository = git('rev-parse', '--show-toplevel').decode().strip()
            commit = git('rev-parse', 'HEAD').decode().strip()
            roots = sorted(git('rev-list', '--max-parents=0', 'HEAD').decode().splitlines())
            repository_identity = 'git-roots-v1:' + hashlib.sha256(json.dumps(roots).encode()).hexdigest()
            names = run(['git', '-C', str(repository), 'ls-files', '--cached', '--others', '--exclude-standard', '-z'], root, dict(os.environ), timeout)[0].decode().split('\0')
        except RuntimeError:
            repository = str(root)
            commit = None
            identity_file = root / '.dctx0ctl/analysis/project-identity'
            if not identity_file.exists():
                identity_file.parent.mkdir(parents=True, exist_ok=True)
                import uuid
                identity_file.write_text('local-project-v1:' + str(uuid.uuid4()))
            repository_identity = identity_file.read_text()
            names = [str(p.relative_to(root)) for p in root.rglob('*') if p.is_file() and not {'target', '.git', '.dctx0ctl'}.intersection(p.relative_to(root).parts)]
        prefix = str(root.relative_to(repository))
        if prefix == '.':
            prefix = ''
        excluded = state_paths(options, prefix)
        files = {}
        for name in sorted(set(names)):
            if not name or '.dctx0ctl' in Path(name).parts or name in excluded:
                continue
            path = Path(repository) / name
            if path.is_symlink():
                raise RuntimeError('Snapshot symlinks unsupported: ' + name)
            if path.is_file():
                files[name] = hashlib.sha256(path.read_bytes()).hexdigest()
        return {'repository': repository_identity, 'project_prefix': prefix,
                'commit': commit, 'files': files, 'snapshot_hash': ''}
    if action == 'collect':
        # Reject uncaptured local inputs before any build.
        metadata = json.loads(run(['cargo', 'metadata', '--offline', '--locked', '--format-version=1'], root, dict(os.environ), timeout)[0])
        try:
            repository = Path(git('rev-parse', '--show-toplevel').decode().strip())
        except RuntimeError:
            repository = root
        for package in metadata['packages']:
            if package.get('source') is None and not Path(package['manifest_path']).is_relative_to(repository):
                raise RuntimeError('External local path dependency unsupported: ' + package['manifest_path'])
        return collect({**options, 'research': True, 'stream': True, 'offline': True, 'repository_root': str(repository)})
    raise RuntimeError('Unknown analysis adapter action')


def materialize(options):
    import shutil
    root = Path(options['root']).resolve()
    env = dict(os.environ)
    def git(*args):
        return run(['git', '--literal-pathspecs', *args], root, env, options.get('timeout_secs', 300))[0]
    repository = Path(git('rev-parse', '--show-toplevel').decode().strip())
    prefix = root.relative_to(repository)
    destination = Path(options['destination']).resolve()
    git('worktree', 'add', '--detach', str(destination), options['commit'])
    if options.get('overlay'):
        visible = set(run(['git', '-C', str(repository), 'ls-files', '--cached', '--others', '--exclude-standard', '-z'], root, env, options.get('timeout_secs', 300))[0].decode().split('\0'))
        original = set(run(['git', '-C', str(repository), 'ls-tree', '-r', '--name-only', '-z', 'HEAD'], root, env, options.get('timeout_secs', 300))[0].decode().split('\0'))
        excluded = state_paths(options, prefix)
        for name in sorted(visible | original):
            if not name or '.dctx0ctl' in Path(name).parts or name in excluded:
                continue
            source = repository / name
            target = destination / name
            if name not in visible:
                if target.is_file() or target.is_symlink():
                    target.unlink()
            elif source.is_symlink():
                raise RuntimeError('Snapshot symlinks unsupported')
            elif source.is_file():
                target.parent.mkdir(parents=True, exist_ok=True)
                shutil.copy2(source, target)
            elif target.is_file():
                target.unlink()
    return str(destination / prefix)


def research_diff(options):
    root = Path(options['root'])
    repository = run(['git', 'rev-parse', '--show-toplevel'], root, dict(os.environ), 30)[0].decode().strip()
    result = coverage_diff({**options, 'root': repository})
    excluded = state_paths(options, root.resolve().relative_to(Path(repository).resolve()))
    result['files'] = [f for f in result['files'] if '.dctx0ctl' not in Path(f['path']).parts and f['path'] not in excluded]
    return result
