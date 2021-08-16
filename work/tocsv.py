import toml
import csv


def write_csv_for_file(filename):
    t = toml.load(open(filename))
    csvfile = open(filename + '.csv', 'w', newline='')
    writer = csv.DictWriter(csvfile, fieldnames=['glyph_id', 'unicode'])
    writer.writeheader()
    for k, v in sorted(t.items()):
        writer.writerow({'glyph_id': k, 'unicode': v})
    csvfile.close()


def write_common_csv_for(filenames):
    common_name = ''.join(filenames)
    csvfile = open(common_name + '.csv', 'w', newline='')
    writer = csv.writer(csvfile)
    header = ['glyph_id'] + [f'unicode_in_{filename}' for filename in filenames]
    writer.writerow(header)
    maps = [toml.load(open(filename)) for filename in filenames]
    keys = set().union(*maps)
    for key in sorted(keys):
        writer.writerow([key] + [m.get(key) for m in maps])
    csvfile.close()


def write_csv_for_look_file(filename):
    t = toml.load(open(filename))
    csvfile = open(filename + '.csv', 'w', newline='')
    writer = csv.DictWriter(csvfile, fieldnames=['glyph_id', 'unicode'])
    writer.writeheader()
    for k, v in sorted(t.items()):
        writer.writerow({'glyph_id': k, 'unicode': v['replacement_text']})
    csvfile.close()


def write_common_csv_for_both_kinds(look_files, later_files, common_name):
    csvfile = open(common_name + '.csv', 'w', newline='')
    writer = csv.writer(csvfile)
    header = ['glyph_id'] + [f'unicode_in_{filename}' for filename in (look_files + later_files)]
    writer.writerow(header)
    maps_look = [toml.load(open(filename)) for filename in look_files]
    maps_later = [toml.load(open(filename)) for filename in later_files]
    # keys = set().union(*maps_look).union(*maps_later)
    keys = ['0000', '0005', '0006', '0007', '0009', '000A', '000B', '000C', '000D', '000E', '000F', '0013', '0014', '0017', '0018', '0019', '001A', '001B', '001C', '001D', '001E', '001F', '0020', '0021', '0022', '0023', '0024', '0025', '0026', '0027', '0028', '0029', '002A', '002B', '002C', '002E', '002F', '0030', '0031', '0032', '0033', '0034', '0036', '0039', '003A', '003B', '003C', '003D', '0040', '0041', '0042', '0044', '0045', '0046', '0047', '0048', '0049', '004B', '004C', '004D', '004F', '0050', '0051', '0054', '0060', '0061', '0068', '006A', '006B', '006C', '006D', '006E', '006F', '0070', '0071', '0072', '0073', '0087', '00B3', '00B4', '00B5', '00B6', '00B7', '00B8', '00B9', '00BA', '00BB', '00BC', '00BE', '00C0', '00C1', '00C3', '00C5', '00C6', '00C7', '00C8', '00C9', '00CA', '00CB', '00CC', '00CD', '00CE', '00CF', '00D0', '00D2', '00D4', '00D5', '00D6', '00D7', '00D8', '00DB', '00DC', '00FD', '00FE', '0100', '0101', '0103', '0104', '0105', '010A', '010B', '010D', '010F', '0110', '0112', '0114', '0115', '0116', '0117', '011B', '011C', '011E', '011F', '0149', '014A', '015B', '015E', '016A', '016C', '0193', '0194', '0195', '0197', '019D', '019E', '019F', '01A0', '01A1', '01A2', '01A3', '01A4', '01A5', '01AD', '01AE', '01AF', '01B1', '01B2', '01B3', '01B6', '01B7', '01B8', '01B9', '01BA', '01BB', '01BC', '01BD', '01BE', '01C2', '01C3', '01C4',
            '01C5', '01C6', '01C7', '01C8', '01C9', '01CA', '01CB', '01CD', '01CF', '01D0', '01D1', '01D3', '01D4', '01D7', '01E3', '01E4', '01E7', '01E8', '01E9', '01EB', '01EC', '01ED', '01EE', '01EF', '01F0', '01F1', '01F3', '01F5', '01F6', '01F7', '01F9', '01FA', '01FB', '01FC', '01FD', '01FE', '01FF', '0200', '0201', '0202', '0203', '0204', '0205', '0206', '0207', '0208', '0209', '020A', '020B', '020C', '020D', '020E', '020F', '0210', '0211', '0212', '0213', '0214', '0215', '0216', '0217', '0218', '0219', '022E', '022F', '0230', '0231', '0232', '0233', '0234', '0235', '0236', '0237', '0238', '0239', '023A', '023B', '023C', '023D', '023E', '023F', '0240', '0241', '0242', '0243', '0244', '0245', '0247', '0248', '0249', '024A', '024B', '024C', '024D', '024E', '024F', '0250', '0251', '0259', '025A', '025C', '025D', '025F', '0260', '0261', '0262', '0263', '0264', '0265', '0266', '0267', '0268', '0269', '026A', '026C', '026D', '026F', '0271', '0291', '0295', '0298', '02A3', '02A4', '02A7', '02A8', '02A9', '02AD', '02B2', '02B3', '02B8', '02BC', '02BF', '02DE', '02E2', '02E3', '02E4', '02E5', '02E6', '02E7', '02E8', '02E9', '02EA', '02EB', '02EC', '02ED', '02EE', '02F2', '02F3', '02F4', '02F5', '02F6', '02F8', '02FA', '02FB', '02FD', '02FF', '0300', '0302', '0305', '0306', '0307', '0308', '0309', '030A', '030B', '030C', '030D', '030E', '030F', '0311']
    for key in sorted(keys):
        writer.writerow(
            [key] +
            [(m[key]['replacement_text'] if key in m else None) for m in maps_look] +
            [m.get(key) for m in maps_later]
        )
    csvfile.close()


if __name__ == '__main__':
    # filenames = 'map-40531-0.toml  map-40532-0.toml  map-40533-0.toml  map-40534-0.toml'.split()

    # filenames = 'map-40531-0.toml  map-40532-0.toml'.split()
    # write_common_csv_for(filenames)

    # write_csv_for_look_file('maps/look/font-40533-0-ASZHUB+Times-Roman.toml')

    write_common_csv_for_both_kinds(
        ['maps/look/font-40531-0-APZKLW+NotoSansDevanagari-Bold.toml', 'maps/look/font-40532-0-ATMSNB+NotoSansDevanagari.toml'],
        ['map-40531-0.toml', 'map-40532-0.toml'],
        'common.toml')
