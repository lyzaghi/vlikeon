"""Finite, inventoried core-system data; user firmware remains a separate vault."""
import re


def admit(core):
    if 'system_assets' not in core:
        if 'system_assets_protocol' in core:
            raise ValueError('System assets need their explicit inventory')
        return []
    values=core['system_assets']
    if (type(core.get('system_assets_protocol')) is not int or core['system_assets_protocol']!=1 or
            not isinstance(values,list) or not 1<=len(values)<=32 or core.get('firmware',[])!=[]):
        raise ValueError('The bundled system-asset contract is unsupported')
    directory=core.get('module','').rsplit('/',1)[0]+'/assets/';names=set();resources=set();spellings={};total=0
    for item in values:
        if not isinstance(item,dict) or set(item)!={'path','resource','bytes','sha256'}:
            raise ValueError('System-asset identity is incomplete')
        name=item['path'];resource=item['resource']
        if (not isinstance(name,str) or not 1<=len(name)<=240 or len(name.split('/'))>4 or any(
                not re.fullmatch(r'[A-Za-z0-9_-][A-Za-z0-9_.-]*',part) or part in {'.','..'} or part.endswith('.') or
                re.match(r'(?i)^(CON|PRN|AUX|NUL|COM[0-9]|LPT[0-9])(?:\.|$)',part) for part in name.split('/')) or
                not isinstance(resource,str) or not resource.startswith(directory) or len(resource)>512 or
                any(not re.fullmatch(r'[A-Za-z0-9_.-]+',part) or part in {'.','..'} for part in resource.split('/')) or
                type(item['bytes']) is not int or not 1<=item['bytes']<=32*1024**2 or not isinstance(item['sha256'],str) or not re.fullmatch(r'[a-f0-9]{64}',item['sha256'])):
            raise ValueError('System-asset path, size or hash is invalid')
        alias=name.lower()
        parts=name.split('/')
        for count in range(1,len(parts)+1):
            spelling='/'.join(parts[:count]);key=spelling.lower()
            if key in spellings and spellings[key]!=spelling:raise ValueError('System asset directory spellings collide')
            spellings[key]=spelling
        if alias in names or resource in resources or any(alias.startswith(other+'/') or other.startswith(alias+'/') for other in names):
            raise ValueError('System assets contain duplicate or colliding paths')
        names.add(alias);resources.add(resource);total+=item['bytes']
        if total>64*1024**2:raise ValueError('System assets exceed their byte budget')
    return values
