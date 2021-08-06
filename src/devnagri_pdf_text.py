import re
from collections import defaultdict, deque
import random
import inspect
from typing import Dict


def unicode_codepoints_for_glyph_id(ttx: str) -> Dict[int, str]:
    """Read the font and return a map.

    Graphs:

    `glyph_name_for_glyph_id`:
    (glyph_id) ---> (glyph name)

    `equivalents`:
                    (glyph name) --> (sequence of codepoints)
                            |------> (sequences of names)      (via ligature substitutions)
                            |------> (equivalent glyph names)  (non-ligature substitutions)

    *   Draw edges from GlyphID to name.
    *   Draw edges from name to codepoint(s).
    *   Draw edges from ligature result to input components.
    *   Draw edges from (single) substitution result to (single) input component.

    Replace nodes that have unique codepoint equivalents by their equivalents, and propagate backwards.
    """
    glyph_name_for_glyph_id = {}
    # Build edges for (glyph_id) -> (glyph name), like:
    #     <GlyphID id="6" name="anusvaradeva"/>
    for s in re.findall(r'<GlyphID id=.*/>', ttx):
        _, glyph_id, _, glyph_name, _ = s.split('"')
        glyph_id = int(glyph_id)
        if glyph_id in glyph_name_for_glyph_id:
            assert glyph_name_for_glyph_id[glyph_id] == glyph_name, (glyph_id, glyph_name, glyph_name_for_glyph_id[glyph_id])
        glyph_name_for_glyph_id[glyph_id] = glyph_name

    # equivalents[u] = set of sequences.
    equivalents = defaultdict(set)
    # When it's a set of only integer sequences, we're done.

    '''
    Eventually we want something like: equivalents[u] = set(IntegerSequence1, IntegerSequence2, ...).

    At an intermediate step we may have: equivalents[u] = set(IntegerSequence1, NameSequence2, ...).
    '''

    def is_integer_sequence(s):
        return all(isinstance(e, int) for e in s)

    def is_done(u):
        return all(is_integer_sequence(s) for s in equivalents[u])

    def concat(xyz):
        "Turns (xs, ys, zs) into xs + ys + zs. Etc."
        for xs in xyz:
            yield from xs

    def first_refresh(s):
        '''Replace the first "done" name in sequence s with its equivalent. See `refresh` to replace all names.'''
        prefix = '    ' * len(inspect.stack(0))
        if is_integer_sequence(s):
            print(f'{prefix}Already an integer sequence: #{s}#.')
            yield s
            return
        print(f'{prefix}Trying to refresh sequence #{s}#:')
        # ok = any(is_done(v) for v in s if isinstance(v, str))
        # if not ok:
        #     print(f'Not yet ready to refresh #{s}#.')
        #     yield s
        #     return
        # Now, suppose s is (Integer, Name, Integer, Integer, Name).
        # Find the first Name `v` in s, and replace `v` by its equivalent integer sequence.
        for i in range(len(s)):
            v = s[i]
            if isinstance(v, int):
                # print(f'{prefix}Not trying to do anything for int #{v}# — there will be a name later.')
                continue
            # print(f'{prefix}Found a name: #{v}# -> #{equivalents[v]}#.')
            assert isinstance(v, str)
            if not is_done(v):
                # print(f'''{prefix}Not trying to do anything for name #{v}# -> {equivalents[v]} — it's not yet done.''')
                continue
            for equivalent in equivalents[v]:
                ret = s[:i] + equivalent + s[i+1:]
                print(f'{prefix}While refreshing sequence #{s}#, expanded name #{v}#->#{equivalents[v]}#, yielding sequence #{ret}')
                yield ret
            return
        print(f'{prefix}Found no names to replace right now in sequence #{s}#.')
        yield s
        return

    def refresh(s):
        '''Replace each "done" name in s with its equivalent.'''
        prefix = '    ' * len(inspect.stack(0))
        print(f'{prefix}Trying to refresh sequence #{s}#:')
        to_refresh = deque()
        to_refresh.append(s)
        while to_refresh:
            s = to_refresh.popleft()
            print(f'{prefix}...so: trying to refresh sequence #{s}#:')
            refreshed = list(first_refresh(s))
            if refreshed == [s]:
                yield s
            else:
                to_refresh.extend(refreshed)

    def full_refresh(ss):
        """Replace a set of sequences by new set of sequences."""
        prefix = '   ' * len(inspect.stack(0))
        changed = True
        print(f'{prefix}Refreshing the set #{ss}#.')
        assert isinstance(ss, set)
        ret = set()
        for s in ss:
            if is_integer_sequence(s):
                ret.add(s)
                continue
            print(f'{prefix}In set #{ss}, refreshing sequence #{s}#.')
            refreshes = list(refresh(s))
            print(f'{prefix}In set #{ss}#, refreshing sequence #{s}# gave #{refreshes}# of size #{len(refreshes)}#.')
            for r in refreshes:
                ret.add(r)
        print(f'{prefix}Having refreshed the set #{ss}#, returning set #{ret}#.')
        return ret

    # to_check = set()  # Nodes that potentially have unique codepoint equivalents.
    reverse = defaultdict(set)

    # Build edges for (glyph_name) -> (codepoints), like:
    #     <map code="0x902" name="anusvaradeva"/><!-- DEVANAGARI SIGN ANUSVARA -->
    not_done = deque()
    for s in re.findall(r'<map code=.*/>', ttx):
        _, codepoint, _, glyph_name, _ = s.split('"')
        assert codepoint.startswith('0x')
        codepoint = int(codepoint[2:], 16)
        equivalents[glyph_name].add(tuple([codepoint]))
        not_done.append(glyph_name)
    # Build edges for (glyph_name) -> (sequences of ligature substitutions), like:
    #     <LigatureSet glyph="dadeva">
    #       <Ligature components="viramadeva,vadeva" glyph="davadeva"/>
    #       <Ligature components="viramadeva,yadeva" glyph="dayadeva"/>
    #     </LigatureSet>
    for s in re.findall(r'<LigatureSet.*?</LigatureSet>', ttx, re.DOTALL):
        initial = s.split('"')[1]  # Example: dadeva
        for t in re.findall(r'<Ligature components=.*/>', s):
            sequence = tuple([initial] + t.split('"')[1].split(','))  # Example: viramadeva,yadeva
            result = t.split('"')[3]  # Example: dayadeva
            equivalents[result].add(sequence)
            for part in sequence:
                reverse[part].add(result)
    # Build edges for (glyph_name) -> (equivalent glyph name), from non-ligature substitutions like:
    #     <Substitution in="ladeva" out="ladevaMAR"/>
    for s in re.findall(r'<Substitution in=.*/>', ttx):
        s_out = s.split('"')[3].split(',')  # Example: ['aadeva','evowelsigndeva']
        s_in = s.split('"')[1].split(',')  # Example: 'odeva'
        if len(s_out) != 1:
            continue
        s_out = s_out[0]
        assert len(s_in) == 1, f'#{s_in}#'
        equivalents[s_out].add(tuple(s_in))
        reverse[s_in[0]].add(s_out)

    print(len(equivalents))
    for u in equivalents:
        if u not in not_done:
            not_done.append(u)
    done = set()
    while not_done:
        u = not_done.popleft()
        if all(is_integer_sequence(s) for s in equivalents[u]):
            done.add(u)
            continue
        print(f'\nChecking {u} -> set {equivalents[u]}...')
        before = equivalents[u]
        after = full_refresh(before)
        if before != after:
            print(f'Changed: #{before} to #{after}#.')
            equivalents[u] = after
        if is_done(u):
            print(f'Done: {u} -> {equivalents[u]}')
            done.add(u)
        else:
            print(f'Not yet done: {u} -> {equivalents[u]}')
            not_done.append(u)

    print(len(equivalents))
    # First print the "done" ones.
    num_done = 0
    for u in equivalents:
        if is_done(u):
            print(u, equivalents[u])
            num_done += 1
    print(f'So, done: {num_done}')
    # Then print the "not done" ones.
    num_not_done = 0
    for u in equivalents:
        if not is_done(u):
            print(u, equivalents[u])
            num_not_done += 1
    print(f'So, not done: {num_not_done}')

    raise Exception('All ok.')


class Font:
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

    def __init__(self, ttx):
        unicode_codepoints_for_glyph_id(ttx)
        # Example:
        #      <map code="0x902" name="anusvaradeva"/><!-- DEVANAGARI SIGN ANUSVARA -->
        self.codepoint_for_name: Dict[str, str] = {s.split('"')[3]: chr(int(s.split('"')[1][2:], 16)) for s in re.findall(r'<map code=.*/>', ttx)}
        # Example:
        #     <GlyphID id="6" name="anusvaradeva"/>
        self.glyph_name_for_id: Dict[int, str] = {int(s.split('"')[1]): s.split('"')[3] for s in re.findall(r'<GlyphID id=.*/>', ttx)}
        self.glyph_id_for_name: Dict[str, int] = {s.split('"')[3]: int(s.split('"')[1]) for s in re.findall(r'<GlyphID id=.*/>', ttx)}

        self.ligature_parts = defaultdict(list)
        # Example:
        #   <LigatureSet glyph="dadeva">
        #     <Ligature components="viramadeva,vadeva" glyph="davadeva"/>
        #     <Ligature components="viramadeva,yadeva" glyph="dayadeva"/>
        #   </LigatureSet>
        for s in re.findall(r'<LigatureSet.*?</LigatureSet>', ttx, re.DOTALL):
            initial = s.split('"')[1]  # Example: dadeva
            for t in re.findall(r'<Ligature components=.*/>', s):
                sequence = t.split('"')[1].split(',')  # Example: viramadeva,yadeva
                result = t.split('"')[3]  # Example: dayadeva
                self.ligature_parts[result].append([initial] + sequence)

        self.adesah = defaultdict(list)
        # Example:
        #  <Substitution in="odeva" out="aadeva,evowelsigndeva"/>
        for s in re.findall(r'<Substitution in=.*/>', ttx):
            s_out = s.split('"')[3].split(',')  # Example: ['aadeva','evowelsigndeva']
            s_in = s.split('"')[1]  # Example: 'odeva'
            if len(s_out) != 1:
                continue
            s_out = s_out[0]
            self.adesah[s_out].append(s_in)
            if len(self.adesah[s_out]) > 1:
                print(f'For {s_out}, multiple choices: {self.adesah[s_out]}')

    def unicode_for_glyph_id(self, glyph_id, depth=0):
        '''The sequence of Unicode scalar values (codepoints) for a given glyph_id.'''
        prefix = '    ' * depth
        print(f'{prefix}For glyph_id {glyph_id}:')
        aksrnam = self.glyph_name_for_id[glyph_id]
        # Case 1: This name has already been mapped directly.
        if aksrnam in self.codepoint_for_name:
            print(f'{prefix}  Directly mapped: {aksrnam} -> {self.codepoint_for_name[aksrnam]}')
            return self.codepoint_for_name[aksrnam], 1
        print(f'{prefix}  Unicode for name {aksrnam} not found directly mapped.')
        # Case 2: This name is the result from a LigatureSet.
        if aksrnam in self.ligature_parts:
            for sequence in self.ligature_parts[aksrnam]:
                print(f'{prefix}  Trying parts: {aksrnam} -> {sequence}')
                part_results = []
                for ligature_part in sequence:
                    part_result = self.unicode_for_glyph_id(self.glyph_id_for_name[ligature_part], depth + 1)
                    print(f'{prefix}  Result for part {ligature_part} is {part_result}')
                    part_results.append(part_result[0])
                print(f'{prefix}part_results: {part_results}')
                return ''.join(part_results), 2
        # Case 3: This name is known as the "out" of a substitution.
        if aksrnam in self.adesah:
            print(f'{prefix}  Trying subst: {aksrnam} -> {self.adesah[aksrnam]}')
            some_in = random.choice(self.adesah[aksrnam])
            if len(self.adesah[aksrnam]) > 1:
                print(f'{prefix}  For glyph_id {glyph_id}={glyph_id:04X} (name {aksrnam}), picking a random choice {some_in} from {self.adesah[aksrnam]}')
            return self.unicode_for_glyph_id(self.glyph_id_for_name[some_in], depth + 1)[0], 3

        return None, 4


def normalize(r):
    r = re.sub(r'ि(([क-ह]्)*[क-ह])', r'\1ि', r)
    r = re.sub(r'(([क-ह]्)*[क-ह][^क-ह]*)र्', r'र्\1', r)
    return r
