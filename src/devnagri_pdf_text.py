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
from typing import Dict


def dprint(*args, **kwargs):
    prefix = '    ' * (len(inspect.stack(0)) - 1)
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

    def is_done(u):
        return all(is_integer_sequence(s) for s in equivalents[u])

    def first_refresh(s):
        '''Replace the first "done" name in sequence s with its equivalent. Use `refresh` to replace all names.'''
        # Now, suppose s is (Integer, Name, Integer, Integer, Name).
        # Find the first Name `v` in s, and replace `v` by each of its equivalent integer sequences.
        for i in range(len(s)):
            v = s[i]
            if isinstance(v, int):
                continue
            assert isinstance(v, str)
            if not is_done(v):
                continue
            for equivalent in equivalents[v]:
                ret = s[:i] + equivalent + s[i+1:]
                dprint(f'While refreshing sequence #{s}#, expanded name #{v}#->#{equivalents[v]}#, yielding sequence #{ret}')
                yield ret
            return
        yield s
        return

    def refresh(s):
        '''Replace each "done" name in s with its equivalent.'''
        dprint(f'Trying to refresh sequence #{s}#:')
        to_refresh = deque()
        to_refresh.append(s)
        while to_refresh:
            s = to_refresh.popleft()
            dprint(f'...so: trying to refresh sequence #{s}#:')
            refreshed = list(first_refresh(s))
            if refreshed == [s]:
                yield s
            else:
                to_refresh.extend(refreshed)

    def full_refresh(ss):
        """Replace a set of sequences by new set of sequences."""
        dprint(f'Refreshing the set #{ss}#.')
        assert isinstance(ss, set)
        ret = set()
        for s in ss:
            if is_integer_sequence(s):
                ret.add(s)
                continue
            dprint(f'In set #{ss}, refreshing sequence #{s}#.')
            refreshes = list(refresh(s))
            dprint(f'In set #{ss}#, refreshing sequence #{s}# gave #{refreshes}# of size #{len(refreshes)}#.')
            for r in refreshes:
                ret.add(r)
        dprint(f'Having refreshed the set #{ss}#, returning set #{ret}#.')
        return ret

    not_done.extend(u for u in equivalents if u not in not_done)
    done = set()
    while not_done:
        u = not_done.popleft()
        if is_done(u):
            done.add(u)
            continue
        dprint(f'Checking {u} -> set {equivalents[u]}...')
        before = equivalents[u]
        after = full_refresh(before)
        if before != after:
            dprint(f'Changed: #{before} to #{after}#.')
            equivalents[u] = after
        if is_done(u):
            dprint(f'Done: {u} -> {equivalents[u]}')
            done.add(u)
        else:
            dprint(f'Not yet done: {u} -> {equivalents[u]}')
            not_done.append(u)

    # Remove the "not done" ones.
    for u in equivalents:
        if not is_done(u):
            print(f'Not done: {u} -> {equivalents[u]}')
            del equivalents[u]

    return (glyph_name_for_glyph_id, equivalents)


def normalize(r):
    r = re.sub(r'ि(([क-ह]्)*[क-ह])', r'\1ि', r)
    r = re.sub(r'(([क-ह]्)*[क-ह][^क-ह]*)र्', r'र्\1', r)
    return r
