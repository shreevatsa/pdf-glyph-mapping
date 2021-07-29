"""Given a file foo.pdf, dump all text-showing operators in it, by font.

THIS HAS A BUG!
The font names across pages are not one-to-one with the actual font.
So we need a PDF parser, unfortunately.

-------

First, run:

    qpdf --qdf --object-streams=disable --no-original-object-ids foo.pdf foo-qdf.pdf

Then, run this file on foo-qdf.pdf.
"""

import sys
import re


def read_file(filename):
    cur_font = None
    open_files = {}
    for line in open(filename, 'rb').readlines():
        if re.match(rb'/F[0-9]* ', line):
            try:
                cur_font = line.split()[0][1:].decode('ascii')
            except UnicodeDecodeError:
                print(f'This line starts with /F but something is wrong: #{line}#')
            if cur_font not in open_files:
                print(f'Opening file for {cur_font}')
                open_files[cur_font] = open(f'Tjs-{cur_font}', 'w')
            continue
        if line.strip().endswith(b' Tj'):
            line = line.strip()
            assert line.startswith(b'<')
            assert line.endswith(b'> Tj')
            # assert re.match('<.*> Tj$', line)
            assert cur_font is not None, line
            s = line[1:-4].decode('ascii')
            assert len(s) % 4 == 0
            open_files[cur_font].write(s + '\n')


if __name__ == '__main__':
    filename = sys.argv[1]
    read_file(filename)
