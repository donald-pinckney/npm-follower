from typing import List
import avc
import json
import sys
import io
import os
import bisect
from huggingface_hub import HfApi, CommitOperationAdd, CommitOperationDelete


def load_operations():
    if len(sys.argv) > 1:
        path = sys.argv[1]
    else:
        a = avc.Avc(False)
        path = a.git_operations_path

    with open(path, "r") as f:
        operations = json.load(f)
    return [avc.VirtualAddOperation.from_json(o) for o in operations]


# class LoggingWrapper(io.BufferedIOBase):
#     def __init__(self, wrappee):
#         self.wrappee = wrappee

#     def __getattr__(self, name):
#         print(f"getattr: {name}")
#         return getattr(self.wrappee, name)

#     def seek(self, offset: int, whence: int = 0) -> int:
#         ret = self.wrappee.seek(offset, whence)
#         print(f"seek: {offset}, {whence}, ret = {ret}")
#         return ret

#     def read(self, size: int = -1) -> bytes:
#         ret = self.wrappee.read(size)
#         print(f"read: {size}, len(ret) = {len(ret)}")
#         return ret

#     def fileno(self) -> int:
#         print("fileno")
#         return self.wrappee.fileno()

#     def truncate(self, size: int | None = ...) -> int:
#         print(f"truncate: {size}")
#         return super().truncate(size)

#     def close(self) -> None:
#         print("close")
#         return self.wrappee.close()

#     @property
#     def closed(self) -> bool:
#         ret = self.wrappee.closed
#         print("closed, ret = ", ret)
#         return ret

#     def readable(self) -> bool:
#         print("readable")
#         return self.wrappee.readable()

#     def writable(self) -> bool:
#         print("writable")
#         return self.wrappee.writable()

#     def seekable(self) -> bool:
#         print("seekable")
#         return self.wrappee.seekable()

#     def isatty(self) -> bool:
#         print("isatty")
#         return self.wrappee.isatty()

#     def flush(self) -> None:
#         print("flush")
#         return self.wrappee.flush()

#     def detach(self) -> None:
#         print("detach")
#         return self.wrappee.detach()

#     def __enter__(self):
#         print("__enter__")
#         return self.wrappee.__enter__()

#     def __exit__(self, exc_type, exc_value, traceback):
#         print("__exit__")
#         return self.wrappee.__exit__(exc_type, exc_value, traceback)

#     def __iter__(self):
#         print("__iter__")
#         return self.wrappee.__iter__()

#     def __next__(self) -> bytes:
#         print("__next__")
#         return self.wrappee.__next__()

#     def readline(self, size=-1):
#         print(f"readline: {size}")
#         return self.wrappee.readline(size)

#     def readlines(self, hint=-1):
#         print(f"readlines: {hint}")
#         return self.wrappee.readlines(hint)

#     def tell(self) -> int:
#         ret = self.wrappee.tell()
#         print("tell, ret = ", ret)
#         return ret

#     def writelines(self, lines):
#         print(f"writelines: {lines}")
#         return self.wrappee.writelines(lines)

#     def readinto(self, b):
#         print(f"readinto: {b}")
#         return self.wrappee.readinto(b)

#     def write(self, b):
#         print(f"write: {b}")
#         return self.wrappee.write(b)

#     def readall(self):
#         print("readall")
#         return self.wrappee.readall()

#     def read1(self, n):
#         print(f"read1: {n}")
#         return self.wrappee.read1(n)

#     def readinto1(self, b):
#         print(f"readinto1: {b}")
#         return self.wrappee.readinto1(b)


class SlicedFileReader(io.BufferedIOBase):
    def __init__(self, sources: List[avc.ReadSource]):
        assert len(sources) > 0
        self.sources = sources
        self.current_source = 0
        self.cumulative_lengths = [0]
        for source in sources:
            self.cumulative_lengths.append(
                self.cumulative_lengths[-1] + source.num_bytes)

        self.f = open(sources[self.current_source].local_path, "rb")
        self.f.seek(sources[self.current_source].offset)

        # self.file = file
        # self.start = start
        # self.end = start + len
        # self.len = len
        # # self.current = start
        # self.f = open(file, "rb")
        # self.f.seek(start)

    def find_source_idx(self, offset):
        i = bisect.bisect_right(self.cumulative_lengths, offset)
        idx = i - 1
        if idx == len(self.cumulative_lengths) - 1:
            return None
        else:
            assert idx >= 0
            assert idx < len(self.sources)
            return idx

    def tell(self) -> int:
        return self.cumulative_lengths[self.current_source] + self.f.tell() - self.sources[self.current_source].offset

    # offset - self.cumulative_lengths[self.current_source] + self.sources[self.current_source].offset = self.f.tell()
    def seek(self, offset: int, whence: int = 0) -> int:
        if whence == 0:
            dst_source_idx = self.find_source_idx(offset)
            if dst_source_idx is None:
                dst_source_idx = len(self.sources) - 1
            if dst_source_idx != self.current_source:
                self.f.close()
                self.f = open(self.sources[dst_source_idx].local_path, "rb")
                self.current_source = dst_source_idx

            source_offset = offset + \
                self.sources[dst_source_idx].offset - \
                self.cumulative_lengths[dst_source_idx]
            new_pos = self.f.seek(source_offset, whence)
            return new_pos + self.cumulative_lengths[dst_source_idx] - self.sources[dst_source_idx].offset
        elif whence == 1:  # TODO: is this bad performance?
            return self.seek(self.tell() + offset, 0)
        elif whence == 2:  # TODO: is this bad performance?
            end_offset = self.cumulative_lengths[-1]
            return self.seek(end_offset + offset, 0)
        else:
            raise ValueError(f"Unknown whence: {whence}")
        # return self.current

    def read(self, size: int = -1) -> bytes:
        cur = self.tell()

        if size == -1:
            size = self.cumulative_lengths[-1] - cur
        else:
            size = min(size, self.cumulative_lengths[-1] - cur)

        num_read = 0
        buffer = bytearray()

        while num_read < size:
            remaining = size - num_read

            current_source_remaining = self.sources[self.current_source].num_bytes + \
                self.sources[self.current_source].offset - \
                self.f.tell()

            assert current_source_remaining > 0
            to_read = min(remaining, current_source_remaining)

            tmp_bytes = self.f.read(to_read)
            assert len(tmp_bytes) == to_read
            num_read += to_read
            buffer.extend(tmp_bytes)

            cur = self.tell()
            cur_source_idx = self.find_source_idx(cur)
            if cur_source_idx is None:
                cur_source_idx = len(self.sources) - 1

            if cur_source_idx != self.current_source:
                self.f.close()
                self.f = open(self.sources[cur_source_idx].local_path, "rb")
                self.f.seek(self.sources[cur_source_idx].offset)
                self.current_source = cur_source_idx

        return buffer  # maybe i need to do bytes(buffer)

    def close(self) -> None:
        return self.f.close()

    @property
    def closed(self) -> bool:
        return self.f.closed


def build_hf_operation(op: avc.DirectAddOperation | avc.ConcatenatingAddOperation) -> CommitOperationAdd:
    if isinstance(op, avc.DirectAddOperation):
        return CommitOperationAdd(op.repo_path, op.local_path)
    elif isinstance(op, avc.ConcatenatingAddOperation):
        return CommitOperationAdd(
            op.repo_path,
            SlicedFileReader(op.sources)
        )
    else:
        raise ValueError(f"Unknown operation type: {op}")


def build_hf_operations(ops: List[avc.DirectAddOperation | avc.ConcatenatingAddOperation]) -> List[CommitOperationAdd]:
    hf_ops = []
    repo_paths = set()
    for op in ops:
        if op.repo_path in repo_paths:
            raise ValueError(f"Duplicate repo path: {op.repo_path}")
        hf_ops.append(build_hf_operation(op))
        repo_paths.add(op.repo_path)
    return hf_ops


def main():

    # f = SlicedFileReader("upload_file.py", 0, 40)
    # print(f.read())

    ops = load_operations()
    # print(ops)
    hf_ops = build_hf_operations(ops)
    # print(hf_ops)

    api = HfApi()
    api.create_commit(
        repo_id="donald-pinckney/npm-follower-tarballs-test",
        repo_type="dataset",
        operations=hf_ops,
        commit_message="test",
    )


if __name__ == "__main__":
    main()
