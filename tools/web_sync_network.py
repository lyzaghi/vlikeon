"""Select only explicit canonical HTTPS audiences for browser sync CSP/config."""
import ipaddress
import json
from pathlib import Path
import re
from urllib.parse import urlsplit

ROOT=Path(__file__).resolve().parents[1]
def policy():
    path=ROOT/'security/web-sync-services.json'
    if path.is_symlink() or getattr(path,'is_junction',lambda:False)() or not path.is_file() or path.stat().st_size>8192:raise ValueError('Browser sync service policy must be a bounded regular source file')
    value=json.loads(path.read_text(encoding='utf-8'))
    if set(value)!={'schema_version','enabled','service_audiences'} or type(value['schema_version'])is not int or value['schema_version']!=1 or type(value['enabled'])is not bool:raise ValueError('Browser sync service policy is malformed')
    audiences=value['service_audiences']
    if not isinstance(audiences,list) or len(audiences)>8 or any(not isinstance(item,str) for item in audiences) or len(set(audiences))!=len(audiences):raise ValueError('Browser sync service inventory exceeds its bound')
    for audience in audiences:
        if len(audience)>512 or not audience.isascii() or any(ord(c)<33 or c in "\"'\\;?#@%" for c in audience):raise ValueError('Browser sync audience is invalid')
        url=urlsplit(audience)
        if url.scheme!='https' or not url.hostname or url.username is not None or url.password is not None or url.query or url.fragment or url.path and not re.fullmatch(r'/(?:[A-Za-z0-9_-]+/)*[A-Za-z0-9_-]+',url.path):raise ValueError('Browser sync needs exact HTTPS origins with optional plain base paths')
        host='['+ipaddress.IPv6Address(url.hostname).compressed+']' if ':' in url.hostname else url.hostname
        if ':' not in url.hostname and (len(host)>253 or any(not re.fullmatch(r'[a-z0-9](?:[a-z0-9-]{0,61}[a-z0-9])?',part) for part in host.split('.'))):raise ValueError('Browser sync DNS identity is not canonical')
        if ':' not in url.hostname and re.fullmatch(r'(?:[0-9]+|0x[0-9a-f]*)',host.split('.')[-1]):
            try:address=ipaddress.IPv4Address(host)
            except ValueError:raise ValueError('Browser sync numeric host must be an ordinary canonical IPv4 address')
            if str(address)!=host:raise ValueError('Browser sync IPv4 identity is not canonical')
        if url.port is not None and not 1<=url.port<=65535:raise ValueError('Browser sync service port is invalid')
        canonical='https://'+host+(':'+str(url.port) if url.port not in {None,443} else '')+url.path
        if canonical!=audience:raise ValueError('Browser sync audience must match canonical browser URL serialization')
    if bool(audiences)!=value['enabled']:raise ValueError('Browser sync enabled state contradicts its service inventory')
    return value
def connect_sources():
    value=policy();return sorted({'https://'+urlsplit(item).netloc for item in value['service_audiences']}) if value['enabled'] else []
def resources(crypto_enabled):
    value=policy()
    if value['enabled'] and not crypto_enabled:raise ValueError('Browser sync network origins require the separately admitted crypto artifact')
    return {'web/sync-services.json':(json.dumps(value,sort_keys=True,separators=(',',':'))+'\n').encode('utf-8')}
