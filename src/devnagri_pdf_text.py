import re
from collections import defaultdict
import random
from typing import Dict


class Font:
    def __init__(self, filename_helper):
        ttx = open(filename_helper).read()
        # Example:
        #     <GlyphID id="6" name="anusvaradeva"/>
        self.glyph_name_for_id: Dict[int, str] = {int(s.split('"')[1]): s.split('"')[3] for s in re.findall(r'<GlyphID id=.*/>', ttx)}
        self.glyph_id_for_name: Dict[str, int] = {s.split('"')[3]: int(s.split('"')[1]) for s in re.findall(r'<GlyphID id=.*/>', ttx)}
        # Example:
        #      <map code="0x902" name="anusvaradeva"/><!-- DEVANAGARI SIGN ANUSVARA -->
        self.codepoint_for_name: Dict[str, str] = {s.split('"')[3]: chr(int(s.split('"')[1][2:], 16)) for s in re.findall(r'<map code=.*/>', ttx)}

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

    @ staticmethod
    def normalize(r):
        r = re.sub(r'ि(([क-ह]्)*[क-ह])', r'\1ि', r)
        r = re.sub(r'(([क-ह]्)*[क-ह][^क-ह]*)र्', r'र्\1', r)
        return r
