"""Conservative Rust lexical view for audit evidence, preserving byte/line offsets."""
import re


class ScanUnknown(ValueError):
    """Syntax cannot be safely classified by this limited audit lexer."""


def lexical_view(source: str) -> str:
    chars = list(source)
    def hide(start, end):
        for index in range(start, end):
            if chars[index] != "\n":
                chars[index] = " "
    i = 0
    while i < len(source):
        start = i
        if source.startswith("//", i):
            end = source.find("\n", i)
            i = len(source) if end < 0 else end
        elif source.startswith("/*", i):
            i += 2
            depth = 1
            while i < len(source) and depth:
                if source.startswith("/*", i):
                    depth += 1
                    i += 2
                elif source.startswith("*/", i):
                    depth -= 1
                    i += 2
                else:
                    i += 1
            if depth:
                raise ScanUnknown("unterminated block comment")
        else:
            raw = re.match(r'(?:br|r)(#*)"', source[i:])
            if raw:
                closing = '"' + raw.group(1)
                end = source.find(closing, i + raw.end())
                if end < 0:
                    raise ScanUnknown("unterminated raw string")
                i = end + len(closing)
            elif source[i] == '"':
                i += 1
                while i < len(source):
                    if source[i] == "\\":
                        i += 2
                    elif source[i] == '"':
                        i += 1
                        break
                    else:
                        i += 1
                else:
                    raise ScanUnknown("unterminated string")
            elif source[i] == "'" and (match := re.match(r"'(?:\\(?:u\{[0-9a-fA-F_]+\}|x[0-9a-fA-F]{2}|.)|[^'\\\n])'", source[i:])):
                i += match.end()
            else:
                i += 1
                continue
        hide(start, i)
    return "".join(chars)


def production_view(source: str) -> str:
    view = lexical_view(source)
    chars = list(view)
    for attribute in re.finditer(r'#\s*\[\s*cfg\s*\((.*?)\)\s*\]', view, re.S):
        condition = attribute.group(1).strip()
        if condition != "test":
            if re.search(r'\btest\b', condition):
                raise ScanUnknown("compound test cfg needs compiler classification")
            continue
        tail = view[attribute.end():]
        item = re.search(r'[;{]', tail)
        if item is None:
            raise ScanUnknown("test item has no body/declaration")
        end = attribute.end() + item.end()
        if item.group() == "{":
            depth = 1
            while end < len(view) and depth:
                depth += (view[end] == "{") - (view[end] == "}")
                end += 1
            if depth:
                raise ScanUnknown("unbalanced test item")
        for index in range(attribute.start(), end):
            if chars[index] != "\n":
                chars[index] = " "
    return "".join(chars)
