import sys
import toml
import unicodedata

PREC = '<CCprec>'
SUCC = '<CCsucc>'
PREC_CODE = -1
SUCC_CODE = 1


def seq_from_t(t):
    if t:
        parts1 = t.split(PREC)
        for (i, part1) in enumerate(parts1):
            if i > 0:
                yield PREC_CODE
            parts2 = part1.split(SUCC)
            for (j, part) in enumerate(parts2):
                if j > 0:
                    yield SUCC_CODE
                for codepoint in part:
                    yield ord(codepoint)


def seq_from_c(c):
    if c:
        for code in c:
            assert isinstance(code, int)
            yield code


def seq_from_d(d):
    if d:
        for desc in d:
            if desc == PREC:
                yield PREC_CODE
            elif desc == SUCC:
                yield SUCC_CODE
            else:
                code = int(desc[:4], 16)
                expected = f'{code:04X} {unicodedata.name(chr(code))}'
                assert desc == expected, (desc, expected)
                yield code


def t_from_seq(seq):
    for code in seq:
        if code == SUCC_CODE:
            yield SUCC
        elif code == PREC_CODE:
            yield PREC
        else:
            yield chr(code)


def c_from_seq(seq):
    yield from seq


def d_from_seq(seq):
    for code in seq:
        if code == PREC_CODE:
            yield PREC
        elif code == SUCC_CODE:
            yield SUCC
        else:
            yield f'{code:04X} {unicodedata.name(chr(code))}'


if __name__ == '__main__':
    toml_filename = sys.argv[1]
    assert toml_filename.endswith('.toml'), toml_filename
    mapping = toml.load(open(toml_filename))
    out_mapping = {}
    for glyph_id_str, replacements in mapping.items():
        assert len(glyph_id_str) == 4, glyph_id_str
        # Hack for being able to run on font-usage/font-*.toml
        if isinstance(replacements, list):
            replacements = {'replacement_codes': replacements}
        t = tuple(seq_from_t(replacements.get('replacement_text')))
        c = tuple(seq_from_c(replacements.get('replacement_codes')))
        d = tuple(seq_from_d(replacements.get('replacement_desc')))
        got = set(l for l in [t, c, d] if l)
        assert len(got) == 1, (t, c, d)
        seq = got.pop()
        out_mapping[glyph_id_str] = {
            'replacement_text': ''.join(t_from_seq(seq)),
            'replacment_codes': list(c_from_seq(seq)),
            'replacement_desc': list(d_from_seq(seq))
        }

    new_out_mapping = {}
    for glyph_id_str, replacements in sorted(out_mapping.items()):
        new_out_mapping[glyph_id_str] = replacements
    out_filename = toml_filename[:-5] + '.fixed.toml'
    toml.dump(new_out_mapping, open(out_filename, 'w'))
