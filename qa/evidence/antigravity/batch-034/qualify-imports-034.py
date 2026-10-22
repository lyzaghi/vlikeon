import importlib.util
import json
import pathlib
import sys

root = pathlib.Path(__file__).resolve().parent
source = root / 'source-04cbd74'
spec = importlib.util.spec_from_file_location('actor_imports', source / 'tools/windows_actor_imports.py')
module = importlib.util.module_from_spec(spec)
spec.loader.exec_module(module)
worker = root / 'build-batch-034/worker/Release/omniplay-native-worker.exe'
imports = module.require_device_free_imports(worker)
assert 'kernel32.dll' in imports, imports
old = root / 'build-batch-029/worker/Release/omniplay-native-worker.exe'
old_imports = module.pe_imports(old)
try:
    module.require_device_free_imports(old)
except ValueError as error:
    assert 'desktop/device DLL' in str(error), str(error)
else:
    raise AssertionError('Desktop worker wrongly admitted')
data = worker.read_bytes()
evidence = root / 'runs/batch-034'
for length in (64, 128, 256):
    fixture = evidence / ('truncated-' + str(length) + '.bin')
    fixture.write_bytes(data[:length])
    try:
        module.require_device_free_imports(fixture)
    except ValueError:
        pass
    else:
        raise AssertionError('Truncated PE admitted: ' + str(length))
print(json.dumps({'device_free_imports': imports, 'rejected_desktop_imports': old_imports,
                  'truncated_rejected': [64, 128, 256]}))
