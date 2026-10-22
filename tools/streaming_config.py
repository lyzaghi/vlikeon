"""Read only explicit pinned-product streaming origins for resource policies."""
import json
import ipaddress
from pathlib import Path
from urllib.parse import urlsplit

ROOT=Path(__file__).resolve().parents[1]
def configuration():
    value=json.loads((ROOT/'security/streaming-channel.json').read_text(encoding='utf-8'))
    expected={'schema_version','enabled','relay_origins','allow_browser_host','allow_remote_input','default_profile'}
    if set(value)!=expected or value['schema_version']!=1 or any(type(value[key])is not bool for key in ['enabled','allow_browser_host','allow_remote_input']) or value['default_profile'] not in {'p720_30','p720_60','p1080_30','p1080_60'}:raise ValueError('Streaming product policy is invalid')
    origins=value['relay_origins']
    if not isinstance(origins,list) or len(origins)>8 or len(set(origins))!=len(origins):raise ValueError('Streaming relay inventory exceeds its bound')
    for origin in origins:
        if not isinstance(origin,str) or len(origin)>256 or not origin.isascii() or any(c.isspace() or ord(c)<33 or c in "'\"\\;" for c in origin):raise ValueError('Streaming relay origin is invalid')
        parsed=urlsplit(origin)
        if parsed.scheme!='https' or not parsed.hostname or parsed.username is not None or parsed.password is not None or parsed.path or parsed.query or parsed.fragment or origin!='https://'+parsed.netloc:raise ValueError('Streaming uses exact HTTPS relay origins')
        if parsed.port is not None and not 1<=parsed.port<=65535:raise ValueError('Streaming relay port is invalid')
        host='['+ipaddress.IPv6Address(parsed.hostname).compressed+']' if ':' in parsed.hostname else parsed.hostname
        canonical='https://'+host+(':'+str(parsed.port) if parsed.port not in {None,443} else '')
        if origin!=canonical:raise ValueError('Streaming relay origins must use their canonical browser serialization')
    if value['enabled'] and not origins:raise ValueError('Enabled streaming requires an explicit relay origin')
    return value
def connect_sources():
    value=configuration();return ' '.join(value['relay_origins']) if value['enabled'] else ''
