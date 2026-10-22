"""Source assembly admission for the worker's bounded user-firmware policy."""
import re

ROLES={
 'snes_spc_ipl':('snes',{64}), 'mega_drive_svp':('mega_drive',{2048}),
 'ps1_bios':('ps1',{524288}), 'ps1_bios_jp':('ps1',{524288}), 'ps1_bios_eu':('ps1',{524288}),
 'nds_bios7':('nds',{16384}), 'nds_bios9':('nds',{4096}), 'nds_firmware':('nds',{131072,262144,524288}),
}

def admit(core):
    policy=core.get('firmware',[])
    if not isinstance(policy,list) or len(policy)>8:raise ValueError('Core firmware policy exceeds its bound')
    if policy and (type(core.get('firmware_protocol')) is not int or core['firmware_protocol']!=1):raise ValueError('Core firmware declarations need worker firmware protocol 1')
    names=set()
    for item in policy:
        if not isinstance(item,dict) or set(item)-{'role','filename','allowed_bytes','required_group','optional'}:raise ValueError('Core firmware declaration has an unsupported field')
        role=item.get('role');name=item.get('filename');sizes=item.get('allowed_bytes')
        if not isinstance(role,str) or role not in ROLES or ROLES[role][0] not in core.get('systems',[]):raise ValueError('Core firmware role disagrees with its systems')
        if not isinstance(name,str) or not re.fullmatch(r'[A-Za-z0-9_-][A-Za-z0-9_.-]{0,63}',name) or name.endswith('.') or name in names:raise ValueError('Core firmware filename is not distinct and bounded')
        if not isinstance(sizes,list) or not sizes or len(sizes)>4 or any(type(size)is not int or size not in ROLES[role][1] for size in sizes) or len(set(sizes))!=len(sizes):raise ValueError('Core firmware sizes disagree with the native vault')
        if 'optional'in item and type(item['optional'])is not bool:raise ValueError('Core firmware optional policy is invalid')
        if 'required_group'in item and (not isinstance(item['required_group'],str) or not re.fullmatch(r'[a-z0-9_]{1,64}',item['required_group']) or item.get('optional')is True):raise ValueError('Core firmware required group is invalid')
        names.add(name)
