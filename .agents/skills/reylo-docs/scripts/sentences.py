"""The prose sentences of MDX files, with their length and joints (commas, colons, semicolons, a
parenthesis), measured against the bar zod.dev's docs set: a median of about 11 words, nine in ten
under 20.

    python3 sentences.py stats <file.mdx>...   # the distribution
    python3 sentences.py flag <file.mdx>...    # and each sentence over 20 words or with 3+ joints
"""
import re, sys, os, statistics, json

def prose(path):
    out = []
    lines = open(path).read().split('\n')
    i = 0
    if lines and lines[0] == '---':
        i = lines.index('---', 1) + 1
    fence = None
    para = []
    def flush():
        if para:
            out.append((start[0], ' '.join(para)))
            para.clear()
    start = [0]
    while i < len(lines):
        l = lines[i]
        m = re.match(r'^\s*(`{3,}|~{3,})', l)
        if fence:
            if m and m.group(1)[0] == fence[0] and len(m.group(1)) >= len(fence): fence = None
            i += 1; continue
        if m:
            flush(); fence = m.group(1); i += 1; continue
        s = l.strip()
        if not s or s.startswith(('#', '|', '<', 'import ', 'export ', '{/*', '>')) and not s.startswith('> ') or re.match(r'^[-*] |^\d+\. ', s) and False:
            flush(); i += 1; continue
        if s.startswith('> '):
            s = s[2:]
            if s.startswith('[!'): i += 1; continue
        if re.match(r'^[-*] |^\d+\. ', s): flush()
        s = re.sub(r'^[-*] |^\d+\. ', '', s)
        if not para: start[0] = i + 1
        para.append(s)
        i += 1
    flush()
    sents = []
    for ln, p in out:
        t = re.sub(r'<[^>]+>', '', p)
        t = re.sub(r'\[([^\]]+)\]\([^)]+\)', r'\1', t)
        t = re.sub(r'`[^`]+`', 'CODE', t)
        t = re.sub(r'\*\*|__', '', t)
        for s in re.split(r'(?<=[.!?:])\s+(?=[A-Z])', t):
            s = s.strip()
            if len(s.split()) < 3: continue
            sents.append((ln, s))
    return sents

def metrics(s):
    words = len(s.split())
    joints = s.count(',') + s.count(';') + 2 * s.count('(') + s.count(' — ') + s.count(' - ') + max(0, s.rstrip(':').count(':'))
    return words, joints

if __name__ == '__main__':
    mode, *paths = sys.argv[1:]
    rows = []
    for p in paths:
        for ln, s in prose(p):
            w, j = metrics(s)
            rows.append((p, ln, w, j, s))
    ws = [r[2] for r in rows]; js = [r[3] for r in rows]
    q = lambda a, f: sorted(a)[int(len(a) * f)]
    print(f'{len(rows)} sentences; words median {statistics.median(ws)}, p75 {q(ws,.75)}, p90 {q(ws,.9)}, max {max(ws)}; joints median {statistics.median(js)}, p90 {q(js,.9)}; >25 words {sum(w>25 for w in ws)/len(ws):.0%}; >=3 joints {sum(j>=3 for j in js)/len(js):.0%}')
    if mode == 'flag':
        lim_w, lim_j = 20, 3
        for p, ln, w, j, s in rows:
            if w > lim_w or j >= lim_j:
                print(f'{os.path.basename(p)}:{ln} w{w} j{j} | {s}')
