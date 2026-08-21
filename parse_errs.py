import re
text=open('/tmp/build_out.txt').read()
text=re.sub(r'\x1b\[[0-9;]*m','',text)
lines=text.split('\n')
out=[]
for i,l in enumerate(lines):
    if 'error[E' in l:
        m2=re.search(r'error\[(E\d+)\]',l)
        loc=None
        for j in range(i+1,min(i+4,len(lines))):
            m=re.search(r'--> (src/[^:]+\.rs):(\d+)',lines[j])
            if m:
                loc=f"{m.group(1)}:{m.group(2)}"
                break
        out.append(f"{loc or '?':36s}\t{m2.group(1)}")
print('\n'.join(sorted(out)))