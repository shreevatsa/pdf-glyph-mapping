# Downloaded from Google Sheets, edited out the first line, and then...
# reader = csv.DictReader(open('from-sheets.csv', newline=''))

import csv
import toml
reader = csv.reader(open('from-sheets.csv', newline=''))
rows = [row for row in reader]
final_bold = {}
final_regular = {}
for (i, row) in enumerate(rows):
    if i == 0:
        continue
    # Columns:
    #     0: glyph_id
    #     1: Bold, from PDF
    #     2: Regular, from PDF
    #     3: Bold, from S
    #     4: Regular, from S
    #     5 and 6: Images
    #     7: Bold, from U
    #     8: Regular, from U
    glyph_id = row[0]
    # Order of preference: Bold 1 > 7 > 3, and Regular; 2 > 8 > 4

    def get_final(cols):
        if cols[0]:
            if cols[1] and cols[2]:
                assert cols[0] == cols[1] == cols[2], (glyph_id, cols[0], cols[1], cols[2])
                return cols[0]
            if not cols[1] and not cols[2]:
                return cols[0]
            if cols[1] and not cols[2]:
                assert cols[0] == cols[1]
                return cols[0]
            if not cols[1] and cols[2]:
                assert cols[0] == cols[2]
                return cols[0]
        else:
            if cols[1] and cols[2]:
                if cols[1] == 'ों' and cols[2] == 'र्<CCprec>े':
                    return cols[1]
                assert cols[1] == cols[2], (cols[1], cols[2])
                return cols[1]
            if cols[1] and not cols[2]:
                return cols[1]
            if not cols[1] and cols[2]:
                return cols[2]
            if not cols[1] and not cols[2]:
                return None
    final_bold[glyph_id] = get_final([row[1], row[7], row[3]])
    final_regular[glyph_id] = get_final([row[2], row[8], row[4]])
    # Manually add these
    final_bold['0003'] = final_regular['0003'] = ' '
    final_bold['0262'] = final_regular['025E'] = ''

toml.dump(final_bold, open('from-csv-bold.toml', 'w'))
toml.dump(final_regular, open('from-csv-regular.toml', 'w'))
