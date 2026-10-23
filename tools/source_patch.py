"""Apply a bounded unified source patch with exact positions and context, no fuzz.

Only existing entries in the supplied in-memory source map may be changed.
This is source generation, with no filesystem paths or patch commands executed.
"""
import re

HEADER=re.compile(r'@@ -(\d+)(?:,(\d+))? \+(\d+)(?:,(\d+))? @@(?:.*)\n')

def apply_exact(sources,patch):
    if not isinstance(patch,str) or len(patch.encode('utf-8'))>2*1024*1024:
        raise ValueError('Source patch exceeds its bound')
    lines=patch.splitlines(keepends=True);position=0;result={};hunks=0
    while position<len(lines):
        if not lines[position].startswith('--- a/') or not lines[position].endswith('\n'):
            raise ValueError('Expected a literal existing-file patch header')
        name=lines[position][6:-1];position+=1
        if position>=len(lines) or lines[position]!='+++ b/'+name+'\n' or name not in sources or name in result:
            raise ValueError('Patch target is missing, repeated or outside the source map')
        position+=1;original=sources[name].splitlines(keepends=True);output=[];cursor=0;file_hunks=0
        while position<len(lines) and lines[position].startswith('@@ '):
            match=HEADER.fullmatch(lines[position])
            if not match:raise ValueError('Invalid source patch hunk')
            old_start,old_count,new_start,new_count=match.groups()
            old_start=int(old_start)-1;new_start=int(new_start)-1
            old_count=int(old_count) if old_count is not None else 1
            new_count=int(new_count) if new_count is not None else 1
            if old_start<cursor or old_start>len(original) or new_start<0:
                raise ValueError('Patch positions do not match the exact source')
            output.extend(original[cursor:old_start]);cursor=old_start
            if len(output)!=new_start:raise ValueError('Patch output position differs from its declaration')
            position+=1;read=written=0
            while position<len(lines) and not lines[position].startswith(('@@ ','--- a/')):
                line=lines[position];position+=1
                if not line or line[0] not in ' +-' or not line.endswith('\n'):
                    raise ValueError('Unsupported source patch line')
                if line[0] in ' -':
                    if cursor>=len(original) or original[cursor]!=line[1:]:
                        raise ValueError('Source patch context changed: '+name)
                    cursor+=1;read+=1
                if line[0] in ' +':output.append(line[1:]);written+=1
            if (read,written)!=(old_count,new_count):raise ValueError('Source patch hunk counts differ')
            file_hunks+=1;hunks+=1
            if hunks>4096:raise ValueError('Too many source patch hunks')
        if not file_hunks:raise ValueError('Patch file has no hunks')
        output.extend(original[cursor:]);result[name]=''.join(output)
    if not result:raise ValueError('Empty source patch')
    return result
