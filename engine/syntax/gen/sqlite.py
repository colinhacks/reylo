#!/usr/bin/env python3
"""Writes `src/sqlite/generated.rs` from SQLite's own sources, which are in the public domain.

    curl -fsSLO https://sqlite.org/2026/sqlite-src-3530400.zip && unzip sqlite-src-3530400.zip
    python3 -I engine/syntax/gen/sqlite.py --sqlite sqlite-src-3530400

It builds SQLite's parser generator, Lemon, and its keyword hash generator, runs them on `parse.y`
as SQLite's build does, and copies out the tables. Those tables are the ones in SQLite's released
amalgamation, which the script checks when `--amalgamation` names its `sqlite3.c`.
"""

import argparse
import hashlib
import os
import re
import shutil
import subprocess
import sys
import tempfile

VERSION = "3.53.4"
PARSE_Y_SHA256 = "19628d51a728421581e9075c7b4fd2c67218c5ad30718c7ae7ab0717c9e93185"
HERE = os.path.dirname(os.path.abspath(__file__))
OUT = os.path.join(HERE, "..", "src", "sqlite", "generated.rs")
TABLES = {
    "yy_action": ("YY_ACTION", "u16"),
    "yy_lookahead": ("YY_LOOKAHEAD", "u16"),
    "yy_shift_ofst": ("YY_SHIFT_OFST", "u16"),
    "yy_reduce_ofst": ("YY_REDUCE_OFST", "i16"),
    "yy_default": ("YY_DEFAULT", "u16"),
    "yyFallback": ("YY_FALLBACK", "u16"),
    "yyRuleInfoLhs": ("YY_RULE_LHS", "u16"),
    "yyRuleInfoNRhs": ("YY_RULE_NRHS", "i8"),
}
CONSTANTS = ["YYNOCODE", "YYWILDCARD", "YYNSTATE", "YYNRULE", "YYNTOKEN", "YY_MAX_SHIFT", "YY_MIN_SHIFTREDUCE", "YY_MAX_SHIFTREDUCE", "YY_ERROR_ACTION", "YY_ACCEPT_ACTION", "YY_NO_ACTION", "YY_MIN_REDUCE", "YY_MAX_REDUCE"]
# The rules whose action refuses a statement the grammar takes.
RULES = {
    "EXPR_VARIABLE": "expr ::= VARIABLE",
    "TERM_QNUMBER": "term ::= QNUMBER",
    "EIDLIST_MORE": "eidlist ::= eidlist COMMA nm collate sortorder",
    "EIDLIST_FIRST": "eidlist ::= nm collate sortorder",
    "TRIDXBY_INDEXED": "tridxby ::= INDEXED BY nm",
    "TRIDXBY_NOT_INDEXED": "tridxby ::= NOT INDEXED",
    "SELECT_WITH": "select ::= WITH wqlist selectnowith",
    "SELECT_WITH_RECURSIVE": "select ::= WITH RECURSIVE wqlist selectnowith",
    "SELECT": "select ::= selectnowith",
}


def integers(text):
    return [int(x, 0) for x in re.findall(r"-?\b(?:0x[0-9a-fA-F]+|\d+)\b", re.sub(r"/\*.*?\*/", "", text, flags=re.S))]


def table(source, name):
    m = re.search(r"static const [^\n]*\b" + name + r"\[\]\s*=\s*\{(.*?)\n\};", source, re.S)
    if not m:
        sys.exit(f"no table {name}")
    return integers(m.group(1))


def constant(source, name):
    m = re.search(r"^#define " + name + r"\s+\(?(\d+)\)?", source, re.M)
    if not m:
        sys.exit(f"no constant {name}")
    return int(m.group(1))


def array(name, kind, values):
    lines = [", ".join(str(v) for v in values[i : i + 24]) for i in range(0, len(values), 24)]
    body = ",\n    ".join(lines)
    return f"pub static {name}: [{kind}; {len(values)}] = [\n    {body},\n];\n"


def run(*command, cwd):
    subprocess.run(command, cwd=cwd, check=True, stdout=subprocess.PIPE, stderr=subprocess.STDOUT)


def keywords(header, tokens):
    text = "".join(re.findall(r"'(.)'", re.search(r"zKWText\[\d+\] = \{(.*?)\};", header, re.S).group(1)))
    lengths = integers(re.search(r"aKWLen\[\d+\] = \{(.*?)\};", header, re.S).group(1))
    offsets = integers(re.search(r"aKWOffset\[\d+\] = \{(.*?)\};", header, re.S).group(1))
    codes = re.findall(r"TK_\w+", re.search(r"aKWCode\[\d+\] = \{(.*?)\};", header, re.S).group(1))
    # The tables number keywords from one, with an unused entry first.
    lengths, offsets = lengths[1:], offsets[1:]
    if not len(lengths) == len(offsets) == len(codes):
        sys.exit("the keyword tables disagree on their length")
    return sorted((text[o : o + n], tokens[c]) for n, o, c in zip(lengths, offsets, codes))


def main():
    args = argparse.ArgumentParser()
    args.add_argument("--sqlite", required=True, help="an unpacked sqlite-src archive")
    args.add_argument("--amalgamation", help="the released sqlite3.c, to check the tables against")
    args = args.parse_args()
    src = os.path.abspath(args.sqlite)
    version = open(os.path.join(src, "VERSION")).read().strip()
    if version != VERSION:
        sys.exit(f"{src} is SQLite {version}, not {VERSION}")
    digest = hashlib.sha256(open(os.path.join(src, "src", "parse.y"), "rb").read()).hexdigest()
    if digest != PARSE_Y_SHA256:
        sys.exit(f"parse.y's SHA-256 is {digest}, expected {PARSE_Y_SHA256}")
    with tempfile.TemporaryDirectory() as work:
        cc = os.environ.get("CC", "cc")
        run(cc, "-O1", "-w", "-o", "lemon", os.path.join(src, "tool", "lemon.c"), cwd=work)
        run(cc, "-O1", "-w", "-o", "mkkeywordhash", os.path.join(src, "tool", "mkkeywordhash.c"), cwd=work)
        shutil.copy(os.path.join(src, "src", "parse.y"), work)
        shutil.copy(os.path.join(src, "tool", "lempar.c"), work)
        run("./lemon", "-S", "parse.y", cwd=work)
        hash_header = subprocess.run(["./mkkeywordhash"], cwd=work, check=True, stdout=subprocess.PIPE).stdout.decode()
        parser = open(os.path.join(work, "parse.c"), errors="replace").read()
        header = open(os.path.join(work, "parse.h")).read()
    if re.search(r"^#define YYERRORSYMBOL", parser, re.M):
        sys.exit("the grammar now has an error symbol, which the driver does not read")
    if args.amalgamation:
        released = open(args.amalgamation, errors="replace").read()
        for name in TABLES:
            if table(released, name) != table(parser, name):
                sys.exit(f"{name} differs from the released amalgamation's")
    tokens = {m.group(1): int(m.group(2)) for m in re.finditer(r"^#define (TK_\w+)\s+(\d+)", header, re.M)}
    names_block = re.search(r"yyTokenName\[\] = \{(.*?)\n\};", parser, re.S).group(1)
    names = re.findall(r'/\*\s*\d+\s*\*/\s*"([^"]*)"', names_block)
    tokenize = open(os.path.join(src, "src", "tokenize.c")).read()
    classes = integers(re.search(r"aiClass\[\] = \{\n#ifdef SQLITE_ASCII\n(.*?)#endif", tokenize, re.S).group(1))
    global_c = open(os.path.join(src, "src", "global.c")).read()
    ctype = integers(re.search(r"sqlite3CtypeMap\[256\] = \{(.*?)\};", global_c, re.S).group(1))
    if len(classes) != 256 or len(ctype) != 256:
        sys.exit("the character tables are not 256 entries long")
    words = keywords(hash_header, tokens)
    out = [f"//! Generated by `gen/sqlite.py` from SQLite {VERSION}'s `parse.y`, `tokenize.c` and `global.c`, which are in the public domain. Do not edit.\n\n"]
    for name in CONSTANTS:
        out.append(f"pub const {name}: u16 = {constant(parser, name)};\n")
    out.append(f"pub const YY_REDUCE_COUNT: u16 = {constant(parser, 'YY_REDUCE_COUNT')};\n\n")
    for name, value in sorted(tokens.items(), key=lambda kv: kv[1]):
        out.append(f"pub const {name}: u16 = {value};\n")
    out.append("\n")
    for i, name in enumerate(names):
        if i >= constant(parser, "YYNTOKEN"):
            out.append(f"pub const SYM_{name.upper()}: u16 = {i};\n")
    out.append("\n")
    rule_block = re.search(r"yyRuleName\[\] = \{(.*?)\n\};", parser, re.S).group(1)
    rules = {text: int(number) for number, text in re.findall(r'/\*\s*(\d+)\s*\*/\s*"([^"]*)"', rule_block)}
    for name, text in RULES.items():
        out.append(f"/// `{text}`\npub const RULE_{name}: usize = {rules[text]};\n")
    out.append("\n")
    for name, (rust, kind) in TABLES.items():
        out.append(array(rust, kind, table(parser, name)))
    out.append(array("AI_CLASS", "u8", classes))
    out.append(array("CTYPE_MAP", "u8", ctype))
    out.append(f"/// Each symbol's name, by its number.\npub static NAMES: [&str; {len(names)}] = [\n")
    out.extend(f'    "{n}",\n' for n in names)
    out.append("];\n\n")
    out.append(f"/// SQLite's keywords, sorted, in capitals, with each one's token.\npub static KEYWORDS: [(&str, u16); {len(words)}] = [\n")
    out.extend(f'    ("{w}", {code}),\n' for w, code in words)
    out.append("];\n")
    os.makedirs(os.path.dirname(OUT), exist_ok=True)
    with open(OUT, "w") as f:
        f.write("".join(out))
    print(f"{len(names)} symbols, {constant(parser, 'YYNRULE')} rules, {constant(parser, 'YYNSTATE')} states, {len(words)} keywords; parse.y {digest}")


main()
