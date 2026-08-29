"""What a terminal sends when a key is pressed."""

ENTER = "\r"
TAB = "\t"
BACKTAB = "\x1b[Z"
ESC = "\x1b"
BACKSPACE = "\x7f"
DELETE = "\x1b[3~"
HOME = "\x1b[H"
END = "\x1b[F"

_ARROWS = {"up": "A", "down": "B", "right": "C", "left": "D"}
_FKEYS = {1: "11", 2: "12", 3: "13", 4: "14", 5: "15", 6: "17", 7: "18", 8: "19"}


def ctrl(letter):
    """Ctrl plus a letter, which is one byte -- and why Ctrl+Shift+X needs kitty."""
    return chr(ord(letter.upper()) - 64)


def alt(text):
    return ESC + text


def _modifier(shift=False, ctrl=False, alt=False):
    return 1 + (1 if shift else 0) + (2 if alt else 0) + (4 if ctrl else 0)


def arrow(direction, shift=False, ctrl=False, alt=False):
    final = _ARROWS[direction]
    mod = _modifier(shift, ctrl, alt)
    return f"\x1b[{final}" if mod == 1 else f"\x1b[1;{mod}{final}"


def home(shift=False, ctrl=False, alt=False):
    mod = _modifier(shift, ctrl, alt)
    return HOME if mod == 1 else f"\x1b[1;{mod}H"


def end(shift=False, ctrl=False, alt=False):
    mod = _modifier(shift, ctrl, alt)
    return END if mod == 1 else f"\x1b[1;{mod}F"


def fkey(n, shift=False, ctrl=False, alt=False):
    code = _FKEYS[n]
    mod = _modifier(shift, ctrl, alt)
    return f"\x1b[{code}~" if mod == 1 else f"\x1b[{code};{mod}~"


def kitty(ch, shift=False, ctrl=False, alt=False):
    """A key in the kitty protocol's CSI-u form, which keeps modifiers a plain
    terminal loses: Ctrl+X and Ctrl+Shift+X are the same byte otherwise."""
    return f"\x1b[{ord(ch)};{_modifier(shift, ctrl, alt)}u"


UP, DOWN, LEFT, RIGHT = (arrow(d) for d in ("up", "down", "left", "right"))
