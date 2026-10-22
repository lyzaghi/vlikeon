"""Admission of bounded finite-choice core option policies; no runtime probing."""
import json
import re

FORBIDDEN = ('jit', 'dynarec', 'cpucore', 'cpu_core', 'fast_memory', 'backend', 'renderer', 'render_mode',
             'rdp-plugin', 'rsp-plugin', 'thread', 'wlan', 'network', 'server', 'sdcard',
             'firmware', 'bios_path', 'sysfile', 'cheat', 'directory', '_path')


def identifier(value):
    return isinstance(value, str) and re.fullmatch(r'[A-Za-z0-9_-]{1,128}', value) and value not in {'__proto__', 'constructor', 'prototype'}


def text(value, bound):
    return isinstance(value, str) and bool(value) and len(value.encode('utf-8')) <= bound and not any(ord(c) < 32 or 127 <= ord(c) <= 159 for c in value)


def option_value(value):
    return isinstance(value, str) and bool(re.fullmatch(r'[\x20-\x7e]{1,128}', value))


def validate_policy(policy):
    if not isinstance(policy, dict) or set(policy) != {'schema_version', 'definitions'} or type(policy['schema_version']) is not int or policy['schema_version'] != 1 or not isinstance(policy['definitions'], list) or len(policy['definitions']) > 32:
        raise ValueError('Core option policies need a supported bounded keyed schema')
    if len(json.dumps(policy, ensure_ascii=False, separators=(',', ':')).encode('utf-8')) > 65536:
        raise ValueError('Core option policy exceeds its control payload bound')
    keys = set()
    total = 0
    for definition in policy['definitions']:
        fields = {'key', 'label_vi', 'label_en', 'description_vi', 'description_en', 'default_value', 'choices'}
        if not isinstance(definition, dict) or set(definition) != fields:
            raise ValueError('Core options need explicit keyed definitions')
        key = definition['key']
        if not identifier(key) or key in keys or any(part in key.lower() for part in FORBIDDEN):
            raise ValueError('Core option keys cannot override fixed execution, network or file policies')
        keys.add(key)
        if any(not text(definition[field], 160) for field in ['label_vi', 'label_en']) or any(not text(definition[field], 512) for field in ['description_vi', 'description_en']):
            raise ValueError('Core option labels/descriptions exceed their bounds')
        choices = definition['choices']
        if not isinstance(choices, list) or not 1 <= len(choices) <= 32:
            raise ValueError('Core options need finite declared choices')
        total += len(choices)
        if total > 256:
            raise ValueError('The core choice count exceeds its bound')
        values = set()
        for choice in choices:
            if not isinstance(choice, dict) or set(choice) != {'value', 'label_vi', 'label_en'} or not option_value(choice['value']) or choice['value'] in values or not text(choice['label_vi'], 160) or not text(choice['label_en'], 160):
                raise ValueError('Core option choices are invalid or duplicated')
            values.add(choice['value'])
        if definition['default_value'] not in values:
            raise ValueError('A core option default must belong to its declared choices')
    return policy


def admit(core):
    if 'core_options_protocol' not in core:
        if 'core_option_definitions' in core:
            raise ValueError('Core options need their explicit installed protocol')
        return
    if type(core['core_options_protocol']) is not int or core['core_options_protocol'] != 1:
        raise ValueError('The installed core option protocol is unsupported')
    policy = validate_policy({'schema_version': 1, 'definitions': core.get('core_option_definitions')})
    if any(d['key'] == 'beetle_psx_enable_memcard1' for d in policy['definitions']) and ('save_files' not in core.get('capabilities', []) or core.get('saved_files_protocol') != 1):
        raise ValueError('A selectable Web second card requires its persistent-file envelope')
