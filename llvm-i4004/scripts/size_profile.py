import re, collections, sys
txt = open(sys.argv[1]).read()
toks = re.findall(r"\b(fim|src|rdm|wrm|rd0|rd1|rd2|rd3|wr0|wr1|wr2|wr3|ldm|ld|xch|add|sub|inc|isz|jcn|jun|jms|bbl|clc|cmc|stc|iac|dac|rar|ral|tcc|tcs|cma|kbp|dcl|daa|fin|jin|nop)\b", txt)
two = {"fim", "jcn", "jun", "jms", "isz", "fin"}
c = collections.Counter(toks)
total = len(toks)
nbytes = sum(2 if t in two else 1 for t in toks)
print("total instrs=%d  est bytes=%d" % (total, nbytes))
for m, n in c.most_common(24):
    b = n * (2 if m in two else 1)
    print("%6d  %-5s %s  ~%dB" % (n, m, "2B" if m in two else "1B", b))
# pattern: count fim+src pairs (pointer setups) and clc+ld+add / clc+ld+sub chains
s = " ".join(toks)
print("---patterns---")
print("fim src (pointer setup)     :", len(re.findall(r"\bfim src\b", s)))
print("src rdm xch (byte read step):", len(re.findall(r"\bsrc rdm xch\b", s)))
print("src ld wrm (byte write step):", len(re.findall(r"\bsrc ld wrm\b", s)))
print("clc ld add (nibble add)     :", len(re.findall(r"\bclc ld add\b", s)))
print("clc ld sub (nibble sub/cmp) :", len(re.findall(r"\bclc ld sub\b", s)))
