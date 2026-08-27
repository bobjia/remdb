#!/usr/bin/env python3
"""Move top-level fns/structs/enums between files, verbatim.

A block starts at the item's signature line (`(pub )?(fn|struct|enum) NAME` followed
by a delimiter) and absorbs contiguous `///` doc comments / `#[...]` attributes above
it. The block ends at the first line that is exactly `}` (top-level closer).
"""
import json
import re
import sys


def find_block(lines, name):
    pat = re.compile(r'^(pub )?(unsafe )?(fn|struct|enum) ' + re.escape(name) + r'[<(\s]')
    for i, line in enumerate(lines):
        if pat.match(line):
            start = i
            while start > 0 and (
                lines[start - 1].startswith('///') or lines[start - 1].startswith('#[')
            ):
                start -= 1
            j = i
            while j < len(lines):
                if lines[j].rstrip() == '}':
                    return start, j
                j += 1
            raise RuntimeError(f'{name}: no closing brace found')
    raise RuntimeError(f'{name}: signature not found')


def main(spec_path):
    spec = json.load(open(spec_path))
    src = spec['source']
    lines = open(src).readlines()
    blocks = []
    for item in spec['moves']:
        s, e = find_block(lines, item['name'])
        blocks.append((s, e, item['name'], item['target']))

    # sanity: no overlaps
    ordered = sorted(blocks)
    for a, b in zip(ordered, ordered[1:]):
        if a[1] >= b[0]:
            raise RuntimeError(f'overlap between {a[2]} and {b[2]}')

    # remove from source bottom-up, appending to targets in spec order
    for s, e, name, target in sorted(blocks, key=lambda x: -x[0]):
        block = lines[s:e + 1]
        del lines[s:e + 1]
        # collapse the resulting double blank line, if any
        if s < len(lines) and s > 0 and lines[s].strip() == '' and lines[s - 1].strip() == '':
            del lines[s]
        with open(target, 'a') as f:
            f.writelines(block)
            f.write('\n')
        print(f'moved {name}: src lines {s + 1}-{e + 1} -> {target}')

    open(src, 'w').writelines(lines)


if __name__ == '__main__':
    main(sys.argv[1])
