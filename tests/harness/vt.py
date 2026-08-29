"""minimal VT100/xterm emulator"""
import codecs
import ctypes
import locale
from dataclasses import dataclass, replace

try:
    locale.setlocale(locale.LC_ALL, "")
except locale.Error:
    locale.setlocale(locale.LC_ALL, "C.UTF-8")

# The call a terminal itself makes to decide how wide a character is.
_libc = ctypes.CDLL(None)
_libc.wcwidth.argtypes = [ctypes.c_wchar]
_libc.wcwidth.restype = ctypes.c_int


def char_width(ch):
    return max(0, _libc.wcwidth(ch))


@dataclass(frozen=True)
class Style:
    fg: str = ""
    bg: str = ""
    bold: bool = False
    dim: bool = False
    italic: bool = False
    underline: bool = False
    reverse: bool = False

    def __bool__(self):
        return self != PLAIN


PLAIN = Style()

_NAMES = ("black", "red", "green", "yellow", "blue", "magenta", "cyan", "white")


def _indexed(n):
    if n < 8:
        return _NAMES[n]
    if n < 16:
        return "bright_" + _NAMES[n - 8]
    return f"c{n}"


@dataclass
class Cell:
    ch: str = " "
    style: Style = PLAIN


class VT:
    def __init__(self, rows=24, cols=80):
        self.rows = rows
        self.cols = cols
        self.grid = [[Cell() for _ in range(cols)] for _ in range(rows)]
        self.cy = 0
        self.cx = 0
        self.saved = (0, 0)
        self.scrollback = []
        self.replies = bytearray()
        self.cursor_reports = 0
        self.style = PLAIN

        self._state = "GROUND"
        self._params = bytearray()
        self._osc = bytearray()
        self._decoder = codecs.getincrementaldecoder("utf-8")(errors="replace")

    def feed(self, data: bytes) -> bytes:
        for byte in data:
            self._byte(byte)
        replies = bytes(self.replies)
        self.replies.clear()
        return replies

    def resize(self, rows, cols):
        old = self.grid
        self.rows, self.cols = rows, cols
        self.grid = [[Cell() for _ in range(cols)] for _ in range(rows)]
        for y in range(min(rows, len(old))):
            for x in range(min(cols, len(old[y]))):
                self.grid[y][x] = old[y][x]
        self.cy = min(self.cy, rows - 1)
        self.cx = min(self.cx, cols)

    def screen_rows(self):
        return [_row_text(row) for row in self.grid]

    def padded_rows(self):
        """Screen rows at full width, so a wrapped line can be told from a short one."""
        return ["".join(cell.ch for cell in row).ljust(self.cols) for row in self.grid]

    def screen(self):
        return "\n".join(self.screen_rows())

    def rows_all(self):
        return [_row_text(row) for row in self.scrollback] + self.screen_rows()

    def text(self):
        return "\n".join(self.rows_all())

    def styled_runs(self):
        """Every (text, style) run on screen and in the scrollback, in order."""
        runs = []
        for row in self.scrollback + self.grid:
            current, text = None, ""
            for cell in row:
                if cell.ch == "":
                    continue
                if current is not None and cell.style == current:
                    text += cell.ch
                else:
                    if current is not None:
                        runs.append((text, current))
                    current, text = cell.style, cell.ch
            if current is not None:
                runs.append((text.rstrip(), current))
        return [(t, s) for t, s in runs if t]

    def styles_of(self, needle):
        """Styles applied to `needle`, wherever it appears."""
        found = []
        for row in self.scrollback + self.grid:
            line = "".join(c.ch for c in row)
            start = line.find(needle)
            while start != -1:
                found.append(row[start].style)
                start = line.find(needle, start + 1)
        return found

    def _byte(self, b):
        if self._state == "GROUND":
            self._ground(b)
        elif self._state == "ESC":
            self._esc(b)
        elif self._state == "CSI":
            self._csi(b)
        else:
            self._osc_byte(b)

    def _ground(self, b):
        if b == 0x1B:
            self._state = "ESC"
        elif b == 0x0D:
            self.cx = 0
        elif b == 0x0A:
            self._newline()
        elif b == 0x08:
            self.cx = max(0, self.cx - 1)
        elif b == 0x09:
            self.cx = min(self.cols, (self.cx // 8 + 1) * 8)
        elif b in (0x07, 0x00):
            pass
        else:
            for ch in self._decoder.decode(bytes([b])):
                self._put(ch)

    def _esc(self, b):
        c = chr(b)
        self._state = "GROUND"
        if c == "[":
            self._params.clear()
            self._state = "CSI"
        elif c == "]":
            self._osc.clear()
            self._state = "OSC"
        elif c == "7":
            self.saved = (self.cy, self.cx)
        elif c == "8":
            self.cy, self.cx = self.saved
        elif c == "M":
            if self.cy == 0:
                self._scroll_down(1)
            else:
                self.cy -= 1

    def _csi(self, b):
        if 0x40 <= b <= 0x7E:
            self._dispatch(chr(b), self._params.decode("latin1"))
            self._state = "GROUND"
        else:
            self._params.append(b)

    def _osc_byte(self, b):
        # Ends at BEL or ST (ESC \).
        if b == 0x07 or (b == 0x5C and self._osc.endswith(b"\x1b")):
            self._state = "GROUND"
        else:
            self._osc.append(b)

    def _dispatch(self, final, params):
        private = params[:1] in ("?", ">", "<", "=")
        body = params[1:] if private else params

        nums = []
        for part in body.split(";"):
            try:
                nums.append(int(part))
            except ValueError:
                nums.append(0)

        def n(i=0):
            return nums[i] if i < len(nums) and nums[i] else 1

        if final == "n" and not private:
            if nums and nums[0] == 6:
                self.replies += f"\x1b[{self.cy + 1};{self.cx + 1}R".encode()
                self.cursor_reports += 1
            elif nums and nums[0] == 5:
                self.replies += b"\x1b[0n"
            return

        # Private modes: bracketed paste, cursor visibility, kitty keyboard.
        if private:
            return

        if final == "m":
            self._sgr(nums if body else [0])
        elif final == "A":
            self.cy = max(0, self.cy - n())
        elif final == "B":
            self.cy = min(self.rows - 1, self.cy + n())
        elif final == "C":
            self.cx = min(self.cols, self.cx + n())
        elif final == "D":
            self.cx = max(0, self.cx - n())
        elif final == "E":
            self.cy, self.cx = min(self.rows - 1, self.cy + n()), 0
        elif final == "F":
            self.cy, self.cx = max(0, self.cy - n()), 0
        elif final == "G":
            self.cx = max(0, min(self.cols - 1, n() - 1))
        elif final in ("H", "f"):
            row = nums[0] if nums and nums[0] else 1
            col = nums[1] if len(nums) > 1 and nums[1] else 1
            self.cy = max(0, min(self.rows - 1, row - 1))
            self.cx = max(0, min(self.cols - 1, col - 1))
        elif final == "J":
            self._erase_display(nums[0] if nums else 0)
        elif final == "K":
            self._erase_line(nums[0] if nums else 0)
        elif final == "S":
            self._scroll_up(n())
        elif final == "T":
            self._scroll_down(n())
        elif final == "L":
            self._insert_lines(n())
        elif final == "M":
            self._delete_lines(n())
        elif final == "P":
            self._delete_chars(n())
        elif final == "@":
            self._insert_chars(n())
        elif final == "s":
            self.saved = (self.cy, self.cx)
        elif final == "u":
            self.cy, self.cx = self.saved

    def _sgr(self, nums):
        i = 0
        while i < len(nums):
            p = nums[i]
            if p == 0:
                self.style = PLAIN
            elif p == 1:
                self.style = replace(self.style, bold=True)
            elif p == 2:
                self.style = replace(self.style, dim=True)
            elif p == 3:
                self.style = replace(self.style, italic=True)
            elif p == 4:
                self.style = replace(self.style, underline=True)
            elif p == 7:
                self.style = replace(self.style, reverse=True)
            elif p == 22:
                self.style = replace(self.style, bold=False, dim=False)
            elif p == 23:
                self.style = replace(self.style, italic=False)
            elif p == 24:
                self.style = replace(self.style, underline=False)
            elif p == 27:
                self.style = replace(self.style, reverse=False)
            elif 30 <= p <= 37:
                self.style = replace(self.style, fg=_NAMES[p - 30])
            elif 90 <= p <= 97:
                self.style = replace(self.style, fg="bright_" + _NAMES[p - 90])
            elif p == 39:
                self.style = replace(self.style, fg="")
            elif 40 <= p <= 47:
                self.style = replace(self.style, bg=_NAMES[p - 40])
            elif 100 <= p <= 107:
                self.style = replace(self.style, bg="bright_" + _NAMES[p - 100])
            elif p == 49:
                self.style = replace(self.style, bg="")
            elif p in (38, 48):
                colour, used = _extended(nums[i + 1 :])
                key = "fg" if p == 38 else "bg"
                self.style = replace(self.style, **{key: colour})
                i += used
            i += 1

    def _put(self, ch):
        width = char_width(ch)
        if width == 0:
            return
        # Deferred wrap: a glyph in the last column leaves the cursor at the
        # margin, and only the next one moves down. Reedline assumes this.
        if self.cx >= self.cols:
            self.cx = 0
            self._newline()
        if self.cx + width > self.cols:
            self.grid[self.cy][self.cx] = Cell(" ", self.style)
            self.cx = 0
            self._newline()
        self.grid[self.cy][self.cx] = Cell(ch, self.style)
        for offset in range(1, width):
            if self.cx + offset < self.cols:
                self.grid[self.cy][self.cx + offset] = Cell("", self.style)
        self.cx += width

    def _newline(self):
        if self.cy >= self.rows - 1:
            self._scroll_up(1)
        else:
            self.cy += 1

    def _blank(self):
        return [Cell() for _ in range(self.cols)]

    def _scroll_up(self, count):
        for _ in range(count):
            self.scrollback.append(self.grid.pop(0))
            self.grid.append(self._blank())

    def _scroll_down(self, count):
        for _ in range(count):
            self.grid.pop()
            self.grid.insert(0, self._blank())

    def _insert_lines(self, count):
        for _ in range(count):
            self.grid.pop()
            self.grid.insert(self.cy, self._blank())

    def _delete_lines(self, count):
        for _ in range(count):
            self.grid.pop(self.cy)
            self.grid.append(self._blank())

    def _insert_chars(self, count):
        row = self.grid[self.cy]
        for _ in range(count):
            row.insert(self.cx, Cell())
            row.pop()

    def _delete_chars(self, count):
        row = self.grid[self.cy]
        for _ in range(count):
            del row[self.cx]
            row.append(Cell())

    def _erase_display(self, mode):
        if mode == 0:
            self._erase_line(0)
            rows = range(self.cy + 1, self.rows)
        elif mode == 1:
            self._erase_line(1)
            rows = range(0, self.cy)
        else:
            rows = range(self.rows)
        for y in rows:
            self.grid[y] = self._blank()

    def _erase_line(self, mode):
        row = self.grid[self.cy]
        if mode == 0:
            span = range(min(self.cx, self.cols), self.cols)
        elif mode == 1:
            span = range(0, min(self.cx + 1, self.cols))
        else:
            span = range(self.cols)
        for x in span:
            row[x] = Cell()


def _extended(rest):
    """A 38/48 colour and how many parameters it consumed."""
    if rest[:1] == [5] and len(rest) >= 2:
        return _indexed(rest[1]), 2
    if rest[:1] == [2] and len(rest) >= 4:
        return f"rgb({rest[1]},{rest[2]},{rest[3]})", 4
    return "", len(rest)


def _row_text(row):
    return "".join(cell.ch for cell in row).rstrip()
