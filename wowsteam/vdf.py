"""Steam VDF handling, adapted from the community WoW Deck Setup 1.6.

Unknown binary types fail closed; text edits retain unrelated bytes.
"""
import re
import struct
import zlib
from pathlib import Path
from .system import atomic_write

T_MAP, T_STR, T_INT, T_FLOAT = 0x00, 0x01, 0x02, 0x03
T_UINT64, T_END, T_INT64 = 0x07, 0x08, 0x0A


class VdfError(Exception):
    pass


def bvdf_loads(data: bytes) -> list:
    """Parse binary VDF into nested [type, key, value] lists (keeps order)."""
    pos = 0

    def cstr() -> str:
        nonlocal pos
        end = data.find(b"\x00", pos)
        if end < 0:
            raise VdfError("shortcuts.vdf has an unterminated string")
        s = data[pos:end].decode("utf-8", "surrogateescape")
        pos = end + 1
        return s

    def read_map() -> list:
        nonlocal pos
        items = []
        while True:
            if pos >= len(data):
                raise VdfError("shortcuts.vdf ended early")
            t = data[pos]
            pos += 1
            if t == T_END:
                return items
            key = cstr()
            if t == T_MAP:
                val = read_map()
            elif t == T_STR:
                val = cstr()
            elif t == T_INT:
                val = struct.unpack_from("<i", data, pos)[0]
                pos += 4
            elif t == T_FLOAT:
                val = struct.unpack_from("<f", data, pos)[0]
                pos += 4
            elif t == T_UINT64:
                val = struct.unpack_from("<Q", data, pos)[0]
                pos += 8
            elif t == T_INT64:
                val = struct.unpack_from("<q", data, pos)[0]
                pos += 8
            else:
                raise VdfError(f"shortcuts.vdf has an unknown field type {t}")
            items.append([t, key, val])

    try:
        result = read_map()
    except (struct.error, RecursionError) as exc:
        raise VdfError("Truncated or excessively nested binary VDF") from exc
    if pos != len(data):
        raise VdfError("Unexpected trailing bytes in shortcuts.vdf")
    return result


def bvdf_dumps(items: list) -> bytes:
    out = bytearray()

    def enc(s: str) -> bytes:
        return s.encode("utf-8", "surrogateescape") + b"\x00"

    def write_map(entries: list) -> None:
        for t, key, val in entries:
            out.append(t)
            out.extend(enc(key))
            if t == T_MAP:
                write_map(val)
            elif t == T_STR:
                out.extend(enc(val))
            elif t == T_INT:
                out.extend(struct.pack("<i", val))
            elif t == T_FLOAT:
                out.extend(struct.pack("<f", val))
            elif t == T_UINT64:
                out.extend(struct.pack("<Q", val))
            elif t == T_INT64:
                out.extend(struct.pack("<q", val))
            else:
                raise VdfError(f"cannot write field type {t}")
        out.append(T_END)

    write_map(items)
    return bytes(out)


def kv_get(items: list, key: str):
    low = key.lower()
    for item in items:
        if item[1].lower() == low:
            return item
    return None


def kv_set(items: list, t: int, key: str, val) -> None:
    item = kv_get(items, key)
    if item is None:
        items.append([t, key, val])
    else:
        item[0] = t
        item[2] = val


def to_signed32(n: int) -> int:
    return n - (1 << 32) if n >= (1 << 31) else n


def new_shortcut_appid(exe: str, name: str, taken: set) -> int:
    """Steam's usual id for a non-Steam tile: crc32(exe + name) with the top bit set."""
    for n in range(1000):
        seed = exe + name + ("" if n == 0 else str(n))
        appid = (zlib.crc32(seed.encode("utf-8")) & 0xFFFFFFFF) | 0x80000000
        if appid not in taken:
            return appid
    raise VdfError("could not pick a free tile id")


def make_shortcut_fields(appid: int, name: str, exe: str, startdir: str,
                         launch: str) -> list:
    return [
        [T_INT, "appid", to_signed32(appid)],
        [T_STR, "AppName", name],
        [T_STR, "Exe", exe],
        [T_STR, "StartDir", startdir],
        [T_STR, "icon", ""],
        [T_STR, "ShortcutPath", ""],
        [T_STR, "LaunchOptions", launch],
        [T_INT, "IsHidden", 0],
        [T_INT, "AllowDesktopConfig", 1],
        [T_INT, "AllowOverlay", 1],
        [T_INT, "OpenVR", 0],
        [T_INT, "Devkit", 0],
        [T_STR, "DevkitGameID", ""],
        [T_INT, "DevkitOverrideAppID", 0],
        [T_INT, "LastPlayTime", 0],
        [T_STR, "FlatpakAppID", ""],
        [T_MAP, "tags", []],
    ]


class Shortcuts:
    """Steam's list of non-Steam tiles for one account."""

    def __init__(self, path: Path):
        self.path = path
        if path.exists():
            self.root = bvdf_loads(path.read_bytes())
        else:
            self.root = [[T_MAP, "shortcuts", []]]
        top = kv_get(self.root, "shortcuts")
        if top is None or top[0] != T_MAP:
            raise VdfError("shortcuts.vdf does not contain a shortcuts list")
        self.entries = top[2]

    def find(self, name: str):
        for entry in self.entries:
            if entry[0] != T_MAP:
                continue
            item = kv_get(entry[2], "appname")
            if item is not None and item[2] == name:
                return entry[2]
        return None

    def appids(self) -> set:
        ids = set()
        for entry in self.entries:
            if entry[0] == T_MAP:
                item = kv_get(entry[2], "appid")
                if item is not None:
                    ids.add(item[2] & 0xFFFFFFFF)
        return ids

    def appid_of(self, name: str):
        fields = self.find(name)
        if fields is None:
            return None
        item = kv_get(fields, "appid")
        return None if item is None else item[2] & 0xFFFFFFFF

    def find_by_appid(self, appid: int):
        for entry in self.entries:
            if entry[0] != T_MAP:
                continue
            item = kv_get(entry[2], "appid")
            if item is not None and (item[2] & 0xFFFFFFFF) == appid:
                return entry[2]
        return None

    def remove(self, appid: int) -> bool:
        for i, entry in enumerate(self.entries):
            if entry[0] != T_MAP:
                continue
            item = kv_get(entry[2], "appid")
            if item is not None and (item[2] & 0xFFFFFFFF) == appid:
                del self.entries[i]
                return True
        return False

    def upsert(self, name: str, exe: str, startdir: str, launch) -> int:
        """Create or update a tile by name. `launch` may be a function of the appid."""
        fields = self.find(name)
        if fields is None:
            appid = new_shortcut_appid(exe, name, self.appids())
            text = launch(appid) if callable(launch) else launch
            fields = make_shortcut_fields(appid, name, exe, startdir, text)
            self.entries.append([T_MAP, str(len(self.entries)), fields])
            return appid
        item = kv_get(fields, "appid")
        if item is None:
            appid = new_shortcut_appid(exe, name, self.appids())
            kv_set(fields, T_INT, "appid", to_signed32(appid))
        else:
            appid = item[2] & 0xFFFFFFFF
        text = launch(appid) if callable(launch) else launch
        kv_set(fields, T_STR, "Exe", exe)
        kv_set(fields, T_STR, "StartDir", startdir)
        kv_set(fields, T_STR, "LaunchOptions", text)
        return appid

    def save(self) -> None:
        for i, entry in enumerate(self.entries):
            entry[1] = str(i)
        self.path.parent.mkdir(parents=True, exist_ok=True)
        atomic_write(self.path, bvdf_dumps(self.root))


# ---------------------------------------------------------------------------
# Text VDF (Steam's config.vdf, where per-tile Proton choices live)
# ---------------------------------------------------------------------------

_ESCAPES = {"n": "\n", "t": "\t", "\\": "\\", '"': '"'}


def vdf_tokens(text: str):
    """Yield (kind, start, end, value); kind is 's', '{' or '}'."""
    i, n = 0, len(text)
    while i < n:
        c = text[i]
        if c in " \t\r\n":
            i += 1
        elif c == "/" and text.startswith("//", i):
            j = text.find("\n", i)
            i = n if j < 0 else j
        elif c == "{" or c == "}":
            yield (c, i, i + 1, None)
            i += 1
        elif c == '"':
            j, buf = i + 1, []
            while j < n:
                ch = text[j]
                if ch == "\\" and j + 1 < n:
                    buf.append(_ESCAPES.get(text[j + 1], "\\" + text[j + 1]))
                    j += 2
                    continue
                if ch == '"':
                    break
                buf.append(ch)
                j += 1
            if j >= n:
                raise VdfError("unterminated quoted string")
            yield ("s", i, j + 1, "".join(buf))
            i = j + 1
        elif c == "[":
            j = text.find("]", i)
            if j < 0:
                raise VdfError("unterminated condition")
            i = j + 1
        else:
            j = i
            while j < n and text[j] not in ' \t\r\n{}"':
                j += 1
            yield ("s", i, j, text[i:j])
            i = j


class VLeaf:
    __slots__ = ("key", "key_start", "value", "end")

    def __init__(self, key, key_start, value, end):
        self.key, self.key_start, self.value, self.end = key, key_start, value, end


class VNode:
    __slots__ = ("key", "key_start", "open_pos", "close_pos", "children")

    def __init__(self, key, key_start, open_pos):
        self.key, self.key_start, self.open_pos = key, key_start, open_pos
        self.close_pos = -1
        self.children = []

    def child(self, name: str):
        low = name.lower()
        for c in self.children:
            if isinstance(c, VNode) and c.key.lower() == low:
                return c
        return None

    def entry(self, name: str):
        low = name.lower()
        for c in self.children:
            if c.key.lower() == low:
                return c
        return None

    def leaf(self, name: str):
        low = name.lower()
        for c in self.children:
            if isinstance(c, VLeaf) and c.key.lower() == low:
                return c.value
        return None


def _parse_into(toks: list, i: int, node: VNode, is_root: bool) -> int:
    n = len(toks)
    while i < n:
        kind, start, end, val = toks[i]
        if kind == "}":
            if is_root:
                raise VdfError("unexpected closing brace")
            node.close_pos = start
            return i + 1
        if kind != "s":
            raise VdfError("unexpected opening brace")
        if i + 1 >= n:
            raise VdfError("key without a value")
        nkind, nstart, nend, nval = toks[i + 1]
        if nkind == "{":
            child = VNode(val, start, nstart)
            i = _parse_into(toks, i + 2, child, False)
            node.children.append(child)
        elif nkind == "s":
            node.children.append(VLeaf(val, start, nval, nend))
            i += 2
        else:
            raise VdfError("unexpected closing brace")
    if not is_root:
        raise VdfError("missing closing brace")
    return i


def vdf_parse_tree(text: str) -> VNode:
    root = VNode("", 0, -1)
    _parse_into(list(vdf_tokens(text)), 0, root, True)
    return root


def find_path(node, names):
    for name in names:
        node = node.child(name) if node is not None else None
    return node


STEAM_SECTION = ["InstallConfigStore", "Software", "Valve", "Steam"]


def _line_start(text: str, pos: int) -> int:
    return text.rfind("\n", 0, pos) + 1


def _compat_entry(indent: str, appid: str, tool: str, priority: str) -> str:
    return (f'{indent}"{appid}"\n'
            f'{indent}{{\n'
            f'{indent}\t"name"\t\t"{tool}"\n'
            f'{indent}\t"config"\t\t""\n'
            f'{indent}\t"priority"\t\t"{priority}"\n'
            f'{indent}}}\n')


def _insert_before_close(text: str, close_pos: int, make_block) -> str:
    start = _line_start(text, close_pos)
    before = text[start:close_pos]
    if before.strip() == "":
        return text[:start] + make_block(before + "\t") + text[start:]
    indent = re.match(r"[ \t]*", text[start:]).group(0)
    return (text[:close_pos] + "\n" + make_block(indent + "\t") + indent
            + text[close_pos:])


def set_compat_tool(text: str, appid: str, tool: str, priority: str = "250") -> str:
    """Return config.vdf text with `appid` mapped to `tool`. Everything else is untouched."""
    root = vdf_parse_tree(text)
    steam = find_path(root, STEAM_SECTION)
    if steam is None:
        raise VdfError("could not find the Steam section in config.vdf")
    ctm = steam.child("CompatToolMapping")
    if ctm is None:
        def block(ind):
            return (f'{ind}"CompatToolMapping"\n{ind}{{\n'
                    + _compat_entry(ind + "\t", appid, tool, priority)
                    + f'{ind}}}\n')
        return _insert_before_close(text, steam.close_pos, block)
    existing = ctm.entry(appid)
    if existing is not None:
        if isinstance(existing, VNode) and existing.leaf("name") == tool and existing.leaf("config") == "" and existing.leaf("priority") == priority:
            return text
        end = existing.close_pos + 1 if isinstance(existing, VNode) else existing.end
        return text[:existing.key_start] + _compat_entry("", appid, tool, priority).rstrip("\n") + text[end:]
    return _insert_before_close(
        text, ctm.close_pos, lambda ind: _compat_entry(ind, appid, tool, priority))


def remove_compat_tool(text: str, appid: str) -> str:
    """Return config.vdf text without a Proton entry for `appid` (unchanged if none)."""
    ctm = find_path(vdf_parse_tree(text), STEAM_SECTION + ["CompatToolMapping"])
    existing = ctm.entry(appid) if ctm is not None else None
    if existing is None:
        return text
    end = existing.close_pos + 1 if isinstance(existing, VNode) else existing.end
    return text[:existing.key_start] + text[end:]


def get_compat_tool(text: str, appid: str):
    ctm = find_path(vdf_parse_tree(text), STEAM_SECTION + ["CompatToolMapping"])
    if ctm is None:
        return None
    entry = ctm.child(appid)
    return None if entry is None else entry.leaf("name")


