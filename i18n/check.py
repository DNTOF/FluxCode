"""Check a translation file against i18n/source.tsv.

    python -I i18n/check.py fr     (or ms)

Every line of i18n/<lang>.tsv is `English<TAB>translation`, in the same order as
source.tsv, with the same English text, the same {placeholders}, and a non-empty
translation. Escapes: \\n for a newline, \\t for a tab, \\\\ for a backslash.
"""
import os
import re
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
lang = sys.argv[1]
src = [l.rstrip("\n").split("\t") for l in open(os.path.join(HERE, "source.tsv"), encoding="utf-8")]
path = os.path.join(HERE, f"{lang}.tsv")
if not os.path.exists(path):
    sys.exit(f"missing {path}")
got = [l.rstrip("\n") for l in open(path, encoding="utf-8")]
errors = []
if len(got) != len(src):
    errors.append(f"{len(got)} lines, expected {len(src)}")
ph = re.compile(r"\{[A-Za-z0-9_]*\}")
for i, (row, line) in enumerate(zip(src, got), 1):
    parts = line.split("\t")
    if len(parts) != 2:
        errors.append(f"line {i}: needs exactly one tab")
        continue
    en, tr = parts
    if en != row[0]:
        errors.append(f"line {i}: English text differs from source.tsv: {en[:60]!r} vs {row[0][:60]!r}")
        continue
    if not tr.strip():
        errors.append(f"line {i}: empty translation")
    if sorted(ph.findall(en)) != sorted(ph.findall(tr)):
        errors.append(f"line {i}: placeholders differ: {ph.findall(en)} vs {ph.findall(tr)}")
    if en.count("\\n") != tr.count("\\n") and "\\n" in en:
        errors.append(f"line {i}: line breaks (\\n) differ")
for e in errors[:60]:
    print(e)
print(f"{lang}: {len(got)} lines, {len(errors)} problems")
sys.exit(1 if errors else 0)
