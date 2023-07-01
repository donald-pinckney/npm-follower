from typing import List, Union
import avc
import json
import sys
import io
import os
import bisect
# import cfut
from more_itertools import chunked
from tqdm.contrib.concurrent import process_map  # or thread_map
# from multiprocessing import Pool
from huggingface_hub import HfApi, CommitOperationAdd, CommitOperationDelete


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

    def __getstate__(self):
        # Copy the object's state from self.__dict__ which contains
        # all our instance attributes. Always use the dict.copy()
        # method to avoid modifying the original state.
        state = self.__dict__.copy()
        # Remove the unpicklable entries.
        state['f_tell'] = self.f.tell()
        del state['f']
        return state

    def __setstate__(self, state):
        # Restore instance attributes.
        f_tell = state['f_tell']
        del state['f_tell']

        self.__dict__.update(state)
        # Restore the previously opened file's state. To do so, we need to
        # reopen it and seek.

        f = open(self.sources[self.current_source].local_path, "rb")
        f.seek(f_tell)

        # Finally, save the file.
        self.f = f

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



def load_operations():
    if len(sys.argv) > 1:
        path = sys.argv[1]
    else:
        a = avc.Avc(data_dir=None, initialize=False)
        path = a.git_operations_path

    with open(path, "r") as f:
        operations = json.load(f)
    operations = [avc.VirtualAddOperation.from_json(o) for o in operations]

    if os.path.isfile('completed_git_operations.json'):
        with open('completed_git_operations.json', 'r') as f:
            completed_ops = [avc.VirtualAddOperation.from_json(oj) for oj in json.load(f)]
    else:
        completed_ops = []

    filtered_out_ops = [o for o in operations if o in completed_ops]
    remaining_ops = [o for o in operations if o not in completed_ops]

    print("Filtered out ops:")
    print(filtered_out_ops)

    print("Remaining ops:")
    print(remaining_ops)
    # sys.exit(0)
    return remaining_ops
    



def build_hf_operation(op: Union[avc.DirectAddOperation, avc.ConcatenatingAddOperation]) -> CommitOperationAdd:
    print("Building:")
    print(op)

    if isinstance(op, avc.DirectAddOperation):
        return CommitOperationAdd(op.repo_path, op.local_path)
    elif isinstance(op, avc.ConcatenatingAddOperation):
        return CommitOperationAdd(
            op.repo_path,
            SlicedFileReader(op.sources)
        )
    else:
        raise ValueError(f"Unknown operation type: {op}")


def build_hf_operations(ops: List[Union[avc.DirectAddOperation, avc.ConcatenatingAddOperation]]) -> List[CommitOperationAdd]:
    hf_ops = list(process_map(build_hf_operation, ops, max_workers=12, chunksize=1))

    return hf_ops

def run_chunks(op_chunks):
    for c in op_chunks:
        run_chunk(c)

        if os.path.isfile('completed_git_operations.json'):
            with open('completed_git_operations.json', 'r') as f:
                completed_ops = [avc.VirtualAddOperation.from_json(oj) for oj in json.load(f)]
        else:
            completed_ops = []
        
        completed_ops += c
        with open('completed_git_operations.json', 'w') as f:
            json.dump([o.to_json() for o in completed_ops], f, indent=4)

            

def run_chunk(op_chunk):
    hf_ops = build_hf_operations(op_chunk)

    api = HfApi()
    api.create_commit(
        repo_id="donald-pinckney/npm-follower-data",
        repo_type="dataset",
        operations=hf_ops,
        commit_message="test",
        num_threads=4
    )

    return 'ok'



def main():
    # f = SlicedFileReader("upload_file.py", 0, 40)
    # print(f.read())

    # sys.path.insert(0, os.path.join(
    #     os.environ['HOME'], "npm-follower/database_exporting"))

    ops = load_operations()
    # print(ops)

    repo_paths = set()
    for op in ops:
        if op.repo_path in repo_paths:
            raise ValueError(f"Duplicate repo path: {op.repo_path}")
        repo_paths.add(op.repo_path)

    # if len(ops) > 20:
    #     sbatch_lines = [
    #         "#SBATCH --time=02:00:00",
    #         "#SBATCH --partition=short",
    #         "#SBATCH --mem=8G",
    #         # This rules out the few nodes that are older than Haswell.
    #         # https://rc-docs.northeastern.edu/en/latest/hardware/hardware_overview.html#using-the-constraint-flag
    #         "#SBATCH --constraint=haswell|broadwell|skylake_avx512|zen2|zen|cascadelake",
    #         f'#SBATCH --cpus-per-task=12',
    #         "module load discovery",
    #         # "export PATH=$PATH:/home/a.guha/bin:/work/arjunguha-research-group/software/bin",
    #     ]

    #     print(f'Will run on {len(ops)} ops.')
    #     op_chunks = list(chunked_or_distributed(
    #         ops, max_groups=30, optimal_group_size=12))
    #     op_chunks = [list(c) for c in op_chunks]
    #     print(
    #         f'Running with {len(op_chunks)} chunks, each of size {len(op_chunks[0])}')

    #     done_count = 0

    #     with cfut.SlurmExecutor(additional_setup_lines=sbatch_lines, additional_import_paths=sys.path, keep_logs=True, debug=True) as executor:
    #         # executor.submit
    #         # for chunk in op_chunks:
    #         #     executor.submit(run_chunk, chunk)
    #         for chunk_result in executor.map(run_chunk, op_chunks):
    #             done_count += 1
    #             print(f"{done_count} / {len(op_chunks)}: {chunk_result}")
    # else:

    op_chunks = list(chunked(ops, 4))
    op_chunks = [list(c) for c in op_chunks]

    run_chunks(op_chunks)


if __name__ == "__main__":
    main()
