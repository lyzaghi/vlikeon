"""Explicit source-bundle admission for UI-owned Emscripten pthread workers."""
import re

FIELDS = {'schema_version', 'api', 'maximum_workers', 'maximum_memory_bytes', 'worker_module', 'worker_type'}
THREAD_FIELDS = {'thread_pool', 'thread_callbacks_protocol', 'pthread_worker_sha256', 'pthread_worker_bytes'}


def admit(core):
    if core.get('wasm_threads') is not True:
        if ('wasm_threads' in core and core['wasm_threads'] is not False) or THREAD_FIELDS.intersection(core):
            raise ValueError('Thread resources require an explicit execution policy')
        return None
    policy = core.get('thread_pool')
    if not isinstance(policy, dict) or set(policy) != FIELDS:
        raise ValueError('Threaded cores need a complete bounded worker policy')
    if (type(policy['schema_version']) is not int or policy['schema_version'] != 1 or
            policy['api'] != 'emscripten_pthreads_ui_v1' or policy['worker_type'] != 'module' or
            type(policy['maximum_workers']) is not int or not 1 <= policy['maximum_workers'] <= 16 or
            type(policy['maximum_memory_bytes']) is not int or
            not 64 * 1024**2 <= policy['maximum_memory_bytes'] <= 1024**3 or policy['maximum_memory_bytes'] % 65536):
        raise ValueError('Threaded core resource bounds are invalid')
    worker = policy['worker_module']
    if (not isinstance(worker, str) or len(worker) > 512 or
            not re.fullmatch(r'cores/[A-Za-z0-9_.-]+/[A-Za-z0-9_.-]+\.js', worker) or
            any(part in {'.', '..'} for part in worker.split('/')) or
            not isinstance(core.get('module'), str) or core['module'].rsplit('/', 1)[0] != worker.rsplit('/', 1)[0]):
        raise ValueError('The pthread module must live beside its core loader')
    if (type(core.get('thread_callbacks_protocol')) is not int or core['thread_callbacks_protocol'] != 1 or
            type(core.get('libretro_result_protocol')) is not int or core['libretro_result_protocol'] != 2 or
            'hardware_video' in core or 'hardware_video_protocol' in core or
            not isinstance(core.get('pthread_worker_sha256'), str) or
            not re.fullmatch(r'[a-f0-9]{64}', core['pthread_worker_sha256']) or
            type(core.get('pthread_worker_bytes')) is not int or not 1 <= core['pthread_worker_bytes'] <= 32 * 1024**2):
        raise ValueError('The pthread callback/result/resource contract is incomplete')
    return policy
