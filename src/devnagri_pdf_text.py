"""
    Search a font's ttx to find Unicode scalar values (codepoints) for a glyph id.

    Basically, the font may contain a CMap table mapping glyph names to Unicode codepoints.
    These would be represented in the ttx dump like:

            <map code="0x902" name="anusvaradeva"/><!-- DEVANAGARI SIGN ANUSVARA -->

    Using this and other information in the file, there are three ways in which we may go
    from (glyph id to) glyph name to codepoints:

    1.  The name is already mapped directly:

            <GlyphID id="6" name="anusvaradeva"/>
            ...
            <map code="0x902" name="anusvaradeva"/><!-- DEVANAGARI SIGN ANUSVARA -->

    2.  The name is the result of a ligature substitution (from the GSUB table):

            <GlyphID id="276" name="baradeva"/>
            ...
            <LigatureSet glyph="badeva">
              <Ligature components="viramadeva,radeva" glyph="baradeva"/>
            </LigatureSet>
            ...
            <map code="0x92c" name="badeva"/><!-- DEVANAGARI LETTER BA -->
            <map code="0x94d" name="viramadeva"/><!-- DEVANAGARI SIGN VIRAMA -->
            <map code="0x930" name="radeva"/><!-- DEVANAGARI LETTER RA -->

    3.  The name is the result of a (non-ligature) substitution (from the GSUB table):

            <GlyphID id="580" name="ladevaMAR"/>
            ...
            <Substitution in="ladeva" out="ladevaMAR"/>
            ...
            <map code="0x932" name="ladeva"/><!-- DEVANAGARI LETTER LA -->
"""

import re
from collections import defaultdict, deque
import inspect


def dprint(*args, **kwargs):
    prefix = '    ' * (len(inspect.stack(0)) - 1)
    if True:
        print(prefix, *args, **kwargs)


def unicode_codepoints_for_glyph_id(ttx: str):
    """Reads a font ttx and returns two maps:

    -   `glyph_name_for_glyph_id` which maps glyph_id to glyph_name, and

    -   `equivalents` which maps glyph_name to a set of sequences of Unicode scalar values.
    """
    glyph_name_for_glyph_id = {}
    # At an intermediate step we may have: equivalents[u] = set(IntegerSequence1, NameSequence2, ...).
    # Eventually we want something like: equivalents[u] = set(IntegerSequence1, IntegerSequence2, ...).
    equivalents = defaultdict(set)

    # Step 1: Build graph:
    # *   Draw edges from GlyphID to name.
    # *   Draw edges from name to codepoint(s).
    # *   Draw edges from ligature result to input components.
    # *   Draw edges from (single) substitution result to(single) input component.
    #
    # That is:
    #
    # `glyph_name_for_glyph_id`:
    # (glyph_id) ----> (glyph name)
    #
    # `equivalents`:
    #                 (glyph name) ---> (sequence of codepoints)
    #                         |-------> (sequences of names)      (via ligature substitutions)
    #                         |-------> (equivalent glyph names)  (non-ligature substitutions)

    # Step 1A: Build edges for (glyph_id) -> (glyph name), like:
    #     <GlyphID id="6" name="anusvaradeva"/>
    for s in re.findall(r'<GlyphID id=.*/>', ttx):
        _, glyph_id, _, glyph_name, _ = s.split('"')
        glyph_id = int(glyph_id)
        if glyph_id in glyph_name_for_glyph_id:
            assert glyph_name_for_glyph_id[glyph_id] == glyph_name, (glyph_id, glyph_name, glyph_name_for_glyph_id[glyph_id])
        glyph_name_for_glyph_id[glyph_id] = glyph_name

    # Step 1B: Build edges for (glyph_name) -> (codepoints), like:
    #     <map code="0x902" name="anusvaradeva"/><!-- DEVANAGARI SIGN ANUSVARA -->
    not_done = deque()
    for s in re.findall(r'<map code=.*/>', ttx):
        _, codepoint, _, glyph_name, _ = s.split('"')
        assert codepoint.startswith('0x')
        codepoint = int(codepoint[2:], 16)
        equivalents[glyph_name].add(tuple([codepoint]))
        not_done.append(glyph_name)
    # Step 1C: Build edges for (glyph_name) -> (sequences of ligature substitutions), like:
    #     <LigatureSet glyph="dadeva">
    #       <Ligature components="viramadeva,vadeva" glyph="davadeva"/>
    #       <Ligature components="viramadeva,yadeva" glyph="dayadeva"/>
    #     </LigatureSet>
    for s in re.findall(r'<LigatureSet.*?</LigatureSet>', ttx, re.DOTALL):
        initial = s.split('"')[1]  # Example: dadeva
        for t in re.findall(r'<Ligature components=.*/>', s):
            sequence = tuple([initial] + t.split('"')[1].split(','))  # Example: viramadeva,yadeva
            result = t.split('"')[3]  # Example: dayadeva
            if 'vattudeva' in sequence or 'uni200D' in sequence or 'dummymarkdeva' in sequence:
                pass
            else:
                equivalents[result].add(sequence)
    # Step 1D: Build edges for (glyph_name) -> (equivalent glyph name), from non-ligature substitutions like:
    #     <Substitution in="ladeva" out="ladevaMAR"/>
    for s in re.findall(r'<Substitution in=.*/>', ttx):
        s_out = s.split('"')[3].split(',')  # Example: ['aadeva','evowelsigndeva']
        s_in = s.split('"')[1].split(',')  # Example: 'odeva'
        if len(s_out) != 1:
            continue
        s_out = s_out[0]
        assert len(s_in) == 1, f'#{s_in}#'
        equivalents[s_out].add(tuple(s_in))

    # Step 2: find names all of whose equivalents are integer sequences, and propagate backwards.
    def is_integer_sequence(s):
        return all(isinstance(e, int) for e in s)

    # `equivalents`:
    #                 (glyph name) ---> (sequence of codepoints)
    #                         |-------> (sequences of names)      (via ligature substitutions)
    #                         |-------> (equivalent glyph names)  (non-ligature substitutions)
    # At an intermediate step we may have: equivalents[u] = set(IntegerSequence1, NameSequence2, ...).
    # Eventually we want something like: equivalents[u] = set(IntegerSequence1, IntegerSequence2, ...).

    def sequences_for(glyph_name):
        ret = set()
        for seq in equivalents[glyph_name]:
            for e in seq:
                if isinstance(e, int):
                    continue
                else:
                    assert isinstance(e, str)
                equivalents[e] = list(sequences_for(e))
                assert all(is_integer_sequence(s) for s in equivalents[e]), (e, equivalents[e])
            # Now what? How do we flatten `seq`? By induction, assume at most one level of nesting:
            # (Int, Int, set([Int, Int, Int], [Int, Int]), Int, ...)
            flat_seq = set()
            n = len(seq)

            def recurse(i, cur):
                if i == n:
                    flat_seq.add(cur)
                    return
                if isinstance(seq[i], int):
                    cur.append(seq[i])
                    recurse(i + 1, cur)
                    cur.pop()
                    return
                # So now it's a set of sequences
                assert isinstance(seq[i], set)
                for e in seq[i]:
                    assert is_integer_sequence(e)
                for e in seq[i]:
                    save = len(cur)
                    cur.extend(e)
                    recurse(i + 1, cur)
                    cur = cur[:save]
            recurse(0)
            ret.union(flat_seq)

        dprint(f'Set sequences_for({glyph_name}) to {ret}')
        equivalents[glyph_name] = ret
        return ret

    return (glyph_name_for_glyph_id, equivalents)


def normalize(r):
    r = re.sub(r'ि(([क-ह]्)*[क-ह])', r'\1ि', r)
    r = re.sub(r'(([क-ह]्)*[क-ह][^क-ह]*)र्', r'र्\1', r)
    return r
