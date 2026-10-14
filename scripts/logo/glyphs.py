"""Minimal TrueType reader: cmap (format 4), hmtx, loca, glyf (simple and
composite glyphs) -> SVG path data in font units (y up)."""
import struct


class Font:
    def __init__(self, path):
        self.d = open(path, "rb").read()
        n = struct.unpack(">H", self.d[4:6])[0]
        self.t = {}
        for i in range(n):
            tag, _, off, ln = struct.unpack(">4sIII", self.d[12 + 16 * i: 28 + 16 * i])
            self.t[tag.decode()] = (off, ln)
        head = self.t["head"][0]
        self.upem = struct.unpack(">H", self.d[head + 18: head + 20])[0]
        self.loca_long = struct.unpack(">h", self.d[head + 50: head + 52])[0] == 1
        maxp = self.t["maxp"][0]
        self.nglyphs = struct.unpack(">H", self.d[maxp + 4: maxp + 6])[0]
        hhea = self.t["hhea"][0]
        self.nhm = struct.unpack(">H", self.d[hhea + 34: hhea + 36])[0]
        self.cmap = self._cmap()

    def u16(self, o): return struct.unpack(">H", self.d[o:o + 2])[0]
    def i16(self, o): return struct.unpack(">h", self.d[o:o + 2])[0]

    def _cmap(self):
        base = self.t["cmap"][0]
        n = self.u16(base + 2)
        for i in range(n):
            pid, eid, off = struct.unpack(">HHI", self.d[base + 4 + 8 * i: base + 12 + 8 * i])
            st = base + off
            if self.u16(st) == 4 and pid in (0, 3):
                segx2 = self.u16(st + 6)
                ends = st + 14
                starts = ends + segx2 + 2
                deltas = starts + segx2
                ranges = deltas + segx2
                m = {}
                for s in range(segx2 // 2):
                    e, b = self.u16(ends + 2 * s), self.u16(starts + 2 * s)
                    dl, ro = self.i16(deltas + 2 * s), self.u16(ranges + 2 * s)
                    for c in range(b, min(e, 0x7f) + 1):
                        if ro == 0:
                            g = (c + dl) & 0xFFFF
                        else:
                            g = self.u16(ranges + 2 * s + ro + 2 * (c - b))
                            if g:
                                g = (g + dl) & 0xFFFF
                        m[chr(c)] = g
                return m
        raise ValueError("no cmap format 4")

    def advance(self, g):
        hm = self.t["hmtx"][0]
        return self.u16(hm + 4 * min(g, self.nhm - 1))

    def _range(self, g):
        loca = self.t["loca"][0]
        if self.loca_long:
            a, b = struct.unpack(">II", self.d[loca + 4 * g: loca + 4 * g + 8])
        else:
            a, b = (2 * x for x in struct.unpack(">HH", self.d[loca + 2 * g: loca + 2 * g + 4]))
        return self.t["glyf"][0] + a, b - a

    def contours(self, g):
        """List of contours; each contour is a list of (x, y, on_curve)."""
        o, ln = self._range(g)
        if ln == 0:
            return []
        nc = self.i16(o)
        if nc >= 0:
            return self._simple(o, nc)
        return self._composite(o)

    def _simple(self, o, nc):
        ends = [self.u16(o + 10 + 2 * i) for i in range(nc)]
        p = o + 10 + 2 * nc
        p += 2 + self.u16(p)  # instructions
        npts = ends[-1] + 1
        flags = []
        while len(flags) < npts:
            f = self.d[p]; p += 1
            flags.append(f)
            if f & 8:
                r = self.d[p]; p += 1
                flags += [f] * r
        xs, ys = [], []
        for axis, short, same in ((xs, 2, 16), (ys, 4, 32)):
            v = 0
            for f in flags:
                if f & short:
                    dv = self.d[p]; p += 1
                    v += dv if f & same else -dv
                elif not f & same:
                    v += self.i16(p); p += 2
                axis.append(v)
        out, s = [], 0
        for e in ends:
            out.append([(xs[i], ys[i], bool(flags[i] & 1)) for i in range(s, e + 1)])
            s = e + 1
        return out

    def _composite(self, o):
        p = o + 10
        out = []
        while True:
            fl, gi = self.u16(p), self.u16(p + 2); p += 4
            if fl & 1:
                dx, dy = self.i16(p), self.i16(p + 2); p += 4
            else:
                dx, dy = struct.unpack(">bb", self.d[p:p + 2]); p += 2
            a = b = c = d = 1.0
            b = c = 0.0
            if fl & 8:
                a = d = self.i16(p) / 16384; p += 2
            elif fl & 0x40:
                a, d = self.i16(p) / 16384, self.i16(p + 2) / 16384; p += 4
            elif fl & 0x80:
                a, b, c, d = (self.i16(p + 2 * k) / 16384 for k in range(4)); p += 8
            if not fl & 2:
                raise ValueError("point-matched composites not supported")
            for ct in self.contours(gi):
                out.append([(a * x + c * y + dx, b * x + d * y + dy, on) for x, y, on in ct])
            if not fl & 0x20:
                return out


def path_data(contours, ox, oy, scale):
    """SVG path for contours placed at (ox, oy) in SVG units, y flipped."""
    def P(x, y):
        return (ox + x * scale, oy - y * scale)
    parts = []
    for ct in contours:
        pts = list(ct)
        # start on an on-curve point (or the midpoint of two off-curve ones)
        if not pts[0][2]:
            if pts[-1][2]:
                pts = pts[-1:] + pts[:-1]
            else:
                mx, my = (pts[0][0] + pts[-1][0]) / 2, (pts[0][1] + pts[-1][1]) / 2
                pts = [(mx, my, True)] + pts
        x0, y0 = P(pts[0][0], pts[0][1])
        seg = ["M%s %s" % (fmt(x0), fmt(y0))]
        pending = None
        for x, y, on in pts[1:] + [pts[0]]:
            if on:
                X, Y = P(x, y)
                if pending:
                    seg.append("Q%s %s %s %s" % (fmt(pending[0]), fmt(pending[1]), fmt(X), fmt(Y)))
                    pending = None
                else:
                    seg.append("L%s %s" % (fmt(X), fmt(Y)))
            else:
                X, Y = P(x, y)
                if pending:
                    mx, my = (pending[0] + X) / 2, (pending[1] + Y) / 2
                    seg.append("Q%s %s %s %s" % (fmt(pending[0]), fmt(pending[1]), fmt(mx), fmt(my)))
                pending = (X, Y)
        seg.append("Z")
        parts.append("".join(seg))
    return "".join(parts)


def bbox(contours, ox, oy, scale):
    xs = [ox + x * scale for ct in contours for x, _, _ in ct]
    ys = [oy - y * scale for ct in contours for _, y, _ in ct]
    return (min(xs), min(ys), max(xs), max(ys)) if xs else None


def fmt(v):
    s = "%.2f" % v
    return s.rstrip("0").rstrip(".") if "." in s else s
