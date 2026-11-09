"""Build the two Windows release files as a delta against one official full EXE."""
from hashlib import sha256
from pathlib import Path
from struct import pack
import argparse
import gzip
import os
import tempfile

FULL = "Jade-Palworld-Toolbox.exe"
FAST = "Jade-Palworld-Toolbox-Fast.exe"
MAGIC = b"JADEDLT1"
# A gear hash forgets bytes older than 32 positions. Insertions/deletions
# therefore do not shift the chunk boundaries throughout the rest of the EXE.
GEAR = tuple(int.from_bytes(sha256(bytes([n])).digest()[:4], "little") for n in range(256))


def chunks(data):
    begin = 0
    while begin < len(data):
        end = min(begin + 1024, len(data))
        limit = min(begin + 24 * 1024, len(data))
        rolling = 0
        while end < limit:
            rolling = ((rolling << 1) + GEAR[data[end]]) & 0xffffffff
            end += 1
            if rolling & 4095 == 0:
                break
        yield begin, end
        begin = end


def emit(stream, target, start, length, source):
    if source is None:
        stream.write(pack("<i", -length))
        stream.write(target[start:start + length])
    else:
        stream.write(pack("<ii", length, source))


def write_target(stream, old_index, target):
    view = memoryview(target)
    start = length = 0
    source = None
    for begin, end in chunks(target):
        offset = old_index.get(sha256(view[begin:end]).digest())
        contiguous = (source is None and offset is None) or (source is not None and offset == source + length)
        if length and not contiguous:
            emit(stream, view, start, length, source)
            length = 0
        if not length:
            start, source = begin, offset
        length += end - begin
    if length:
        emit(stream, view, start, length, source)
    stream.write(pack("<i", 0))


def build(base, directory):
    if not 0 < base.stat().st_size <= 512 * 1024 * 1024:
        raise ValueError("Invalid full EXE baseline size")
    old = base.read_bytes()
    digest = sha256(old).hexdigest()
    target_path = directory / ("Jade-Delta-from-" + digest + ".jdp")
    view = memoryview(old)
    index = {sha256(view[a:b]).digest(): a for a, b in chunks(old)}
    fd, temp_name = tempfile.mkstemp(dir=directory, suffix=".partial")
    temp = Path(temp_name)
    try:
        with os.fdopen(fd, "wb") as raw:
            raw.write(MAGIC)
            with gzip.GzipFile(fileobj=raw, filename="", mode="wb", compresslevel=6, mtime=0) as payload:
                for name in (FULL, FAST):
                    write_target(payload, index, (directory / name).read_bytes())
        total = sum((directory / name).stat().st_size for name in (FULL, FAST))
        if temp.stat().st_size >= total:
            target_path.unlink(missing_ok=True)
            print(f"Skipping delta for {digest}: downloading both complete files is smaller.")
            return
        temp.replace(target_path)
        print(f"{target_path.name}: {target_path.stat().st_size:,} / {total:,} bytes")
    finally:
        temp.unlink(missing_ok=True)


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("base", type=Path)
    parser.add_argument("directory", type=Path)
    args = parser.parse_args()
    build(args.base, args.directory)
