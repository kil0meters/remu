#!/usr/bin/env python3

import sys

class Range():
    def __init__(self, str_in):
        out = str_in.split(":")

        if len(out) == 2:
            self.start = int(out[1])
            self.end = int(out[0])
        else:
            self.start = int(out[0])
            self.end = self.start

    def __len__(self):
        return self.end - self.start + 1

    def __str__(self):
        if self.start == self.end:
            return f"{self.start}"
        else:
            return f"{self.end}:{self.start}"

    def generate_mask(self, offset):
        l = len(self)
        base_str = f"(inst & 0b{'1' * l}{'0' * (offset)})"

        if self.start > offset:
            return f"| {base_str} << {self.start - offset} // imm[{self}]"
        elif offset > self.start:
            return f"| {base_str} >> {offset - self.start} // imm[{self}]"
        else:
            return f"| {base_str} // imm[{self}]"

# input format:
# 10=8|4:3 2=7:6|2:1|5
#
# output:

for arg in sys.argv[1:]:
    start, end = arg.split("=")

    bit_offset = int(start)

    ranges = end.split("|")
    ranges.reverse()

    depth = int(start)

    for r in ranges:
        r = Range(r)

        print(r.generate_mask(depth))
        depth += len(r);

