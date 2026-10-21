# Minimal stdio driver for the civil-mcp server (used by scripts/terrain_batch.py).
import json,subprocess
class M:
    def __init__(s):
        s.p=subprocess.Popen(['./target/release/civil-mcp'],stdin=subprocess.PIPE,stdout=subprocess.PIPE,text=True);s.i=0
        s.p.stdin.write(json.dumps({"jsonrpc":"2.0","id":0,"method":"initialize","params":{"protocolVersion":"2024-11-05","capabilities":{},"clientInfo":{"name":"t","version":"1"}}})+"\n");s.p.stdin.flush();s.p.stdout.readline()
    def call(s,n,a):
        s.i+=1;s.p.stdin.write(json.dumps({"jsonrpc":"2.0","id":s.i,"method":"tools/call","params":{"name":n,"arguments":a}})+"\n");s.p.stdin.flush()
        r=json.loads(s.p.stdout.readline());c=r['result']['content'][0]['text']
        if r['result'].get('isError'): print('ERR',n,c);return None
        return json.loads(c) if c.startswith('{') else c
