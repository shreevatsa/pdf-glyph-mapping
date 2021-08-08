"""See Makefile (doit.sh) for how this is meant to be called.
"""

import random
import re
import sys
from typing import Any, List, TypeVar
import unicodedata
from collections import defaultdict

import devnagri_pdf_text

T = TypeVar('T')


def split_list(big_list: List[T], delimiter: T) -> List[List[T]]:
    """Like string.split(foo), except for lists."""
    cur_list: List[T] = []
    parts: List[List[T]] = []
    for item in big_list:
        if item == delimiter:
            if cur_list:
                parts.append(cur_list)
                cur_list = []
        else:
            cur_list.append(item)
    if cur_list:
        parts.append(cur_list)
    return parts


class HtmlWriter:
    def __init__(self, font_id, font_name, filename_helper, num_glyphs, to_unicode) -> None:
        self.font_id = font_id
        self.font_name = font_name
        self.helper_glyph_name_for_glyph_id, self.helper_sequences_for_name = devnagri_pdf_text.unicode_codepoints_for_glyph_id(open(filename_helper).read()) if filename_helper else None
        self.num_glyphs = num_glyphs
        self.to_unicode = to_unicode
        self.added = 0

        self.html = r'''
<!doctype html>
<html>
<style>
* {
    box-sizing: border-box;
}
.glyph-main {
    background-color:white;
    border:1px dotted red;
}
.glyph-other {
    background-color: #666666;
    border: 1px dotted #666666;
}
</style>
<body>
''' + rf'''
<h1>{font_name}</h1>
<p>Using helper font {filename_helper}.</p>
<dl>'''
        self.footer = r'''
</body>
</html>'''

    def img_for_glyph(self, main_glyph, glyph):
        filename = f'font-{self.font_id}-glyph-{glyph}.png'
        classname = "glyph-main" if glyph == main_glyph else "glyph-other"
        return f'<img title="{glyph}" src="{filename}" class="{classname}"/>'

    def run(self, main_glyph, sample_run):
        return f'<dd>{"".join(self.img_for_glyph(main_glyph, glyph) for glyph in sample_run)}</dd>'

    def add(self, glyph_id_str, times_seen, sample_runs):
        uni = to_unicode.get(glyph_id_str)
        if uni:
            c = chr(int(uni, 16))
            name = unicodedata.name(c)
            mapped_pdf = f'mapped in the PDF to 0x{uni} = {c} = {name}.'
        else:
            mapped_pdf = f'Not mapped in the PDF.'
        if self.helper_glyph_name_for_glyph_id:
            glyph_id = int(glyph_id_str, 16)
            name = self.helper_glyph_name_for_glyph_id.get(glyph_id)
            if name:
                mapped_helper = f'Mapped using the helper font to name {name}:'
                sequences = self.helper_sequences_for_name.get(name, [])
                mapped_helper_sequences = []
                for sequence in sequences:
                    as_str = ''.join(chr(c) for c in sequence)
                    as_names = ' followed by '.join(
                        f'{c:04X} (={unicodedata.name(chr(c))})' for c in sequence)
                    mapped_helper_sequences.append(f'{as_str} ({as_names})')
            else:
                mapped_helper = f'(no name in helper for {glyph_id_str})'
                mapped_helper_sequences = []
        else:
            mapped_helper = '(no helper)'
            mapped_helper_sequences = []
        self.added += 1
        self.html += fr'''
<hr>
<dt>
  <p>Glyph ID {glyph_id_str} (Seen {times_seen} times; glyph {self.added} of {self.num_glyphs})</p>
  <p>{mapped_pdf}</p>
  <p>{mapped_helper} {f'({len(mapped_helper_sequences)} sequences)' if len(mapped_helper_sequences) > 1 else ''}</p>
  {chr(10).join('<li>' + sequence + '</li>' for sequence in mapped_helper_sequences)}
</dt>
{chr(10).join(self.run(glyph_id_str, sample_run) for sample_run in sample_runs)}
<hr>
</div>
        '''


if __name__ == '__main__':
    random.seed(42)
    filename_font = sys.argv[1]  # E.g. font-40532.ttf
    filename_helper = sys.argv[2] if len(sys.argv) > 2 else None  # E.g. noto.ttx
    font_id = re.search(f'font-([0-9]*).ttf', filename_font).group(1)
    filename_tjs = f"Tjs-{font_id}-0"

    def read_mapping(filename):
        to_unicode = {}
        name = None
        for line in open(filename).readlines():
            if not name:
                name = line.strip()
                continue
            groups = re.match('(.*) -> {(.*)}', line).groups()
            to_unicode[groups[0]] = groups[1]
        return name, to_unicode
    fontname, to_unicode = read_mapping(filename_tjs + '.map')

    lines = open(filename_tjs).readlines()
    samples_max = 20
    reservoir = defaultdict(list)  # A few samples for each glyph.
    seen = defaultdict(int)  # How many times each glyph was seen.
    for (n, line) in enumerate(lines):
        if n > 0 and n % 100000 == 0:
            print(f'({n * 100.0 / len(lines):05.2f}%) Done {n:7} lines of {len(lines)}.')
        # s = line.strip()
        # assert len(s) % 4 == 0
        # all_parts = [s[i:i+4] for i in range(0, len(s), 4)]
        s = line.strip().split()
        assert all(len(g) == 4 for g in s)
        all_parts = s
        actual_parts = split_list(all_parts, '0003')
        for parts in actual_parts:
            for glyph_id_str in parts:
                seen[glyph_id_str] += 1
                r = reservoir[glyph_id_str]
                # The second condition here makes it slightly different from the usual.
                if len(r) < samples_max:
                    if parts not in r:
                        r.append(parts)
                else:
                    m = random.randrange(0, seen[glyph_id_str])
                    # If seen[glyph] = N, then m is uniformly distributed over the N values [0..N).
                    # So the probability that m < samples_max is samples_max / N,
                    # which is precisely what we want.
                    if m < samples_max and parts not in r:
                        r[m] = parts

    h = HtmlWriter(font_id, fontname, filename_helper, len(seen), to_unicode)
    for glyph_id_str in sorted(seen, key=lambda k: (seen[k], k), reverse=True):
        h.add(glyph_id_str, seen[glyph_id_str], reservoir[glyph_id_str])
    open(filename_font + '.html', 'w').write(h.html + h.footer)
