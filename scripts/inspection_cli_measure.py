"""Record one release inspect/attest invocation without changing its output or exit code."""
import json
from pathlib import Path
import sys
from release_support import measured

if __name__ == '__main__':
    destination, phase, *command = sys.argv[1:]
    input_path = Path(command[command.index('--file') + 1])
    input_bytes = input_path.stat().st_size
    sample, output = measured(command, Path.cwd(), timeout=300)
    with Path(destination).open('a') as evidence:
        evidence.write(json.dumps({'phase': phase, 'input_bytes': input_bytes, 'command': command, **sample}) + '\n')
    sys.stdout.write(output)
    sys.exit(sample['exit_code'])
