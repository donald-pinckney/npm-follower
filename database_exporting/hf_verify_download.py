import shutil
from huggingface_hub import HfApi, hf_hub_url, HfFolder
import sys
import os
import subprocess
from tqdm.contrib.concurrent import process_map
import hashlib


REPO_ID = "nuprl/npm-follower-data"
REVISION = "main"
# REVISION = "v1.0.0-apr-17-2023"


def file_sha256sum(filename):
    h = hashlib.sha256()
    b = bytearray(128*1024)
    mv = memoryview(b)
    with open(filename, 'rb', buffering=0) as f:
        while n := f.readinto(mv):
            h.update(mv[:n])
    return h.hexdigest()


def check_valid(rf_and_local):
    rf, dst_path = rf_and_local

    if rf.lfs is not None:
        assert 'size' in rf.lfs
        assert rf.lfs['size'] == rf.size

    num_bytes = rf.size

    if not os.path.exists(dst_path):
        print(f"File {dst_path} does not exist")
        src_url = hf_hub_url(REPO_ID, filename=rf.rfilename,
                             repo_type="dataset", revision=REVISION)
        return (src_url, rf.rfilename)

    disk_bytes = os.path.getsize(dst_path)
    if num_bytes != disk_bytes:
        print(
            f"Incorrect number of bytes downloaded for {dst_path}. Should be {num_bytes} bytes instead of {disk_bytes}")
        src_url = hf_hub_url(REPO_ID, filename=rf.rfilename,
                             repo_type="dataset", revision=REVISION)
        return (src_url, rf.rfilename)

    if rf.lfs is not None:
        hf_sha_hex = rf.lfs['sha256']
        disk_sha_hex = file_sha256sum(dst_path)
        backup_sha_bytes = b"__danger__ __bug__ [try 0] should be sha = " + \
            disk_sha_hex.encode()
        backup_sha_hex = hashlib.sha256(backup_sha_bytes).hexdigest()

        if disk_sha_hex != hf_sha_hex:
            print(
                f"({dst_path}): On-disk SHA of {disk_sha_hex} does not match HF SHA of {hf_sha_hex}")
            print(
                f"({dst_path}): Checking backup SHA instead: sha256({backup_sha_bytes}) = {backup_sha_hex}")
            if backup_sha_hex != hf_sha_hex:
                print(
                    f"({dst_path}): Backup SHA is also wrong. Need to re-download")
                src_url = hf_hub_url(REPO_ID, filename=rf.rfilename,
                                     repo_type="dataset", revision=REVISION)
                return (src_url, rf.rfilename)

    return None


def main(dst_dir):
    api = HfApi()
    data_info = api.dataset_info(
        repo_id=REPO_ID, revision=REVISION, files_metadata=True)
    repo_files = data_info.siblings
    # print(repo_files)

    rf_and_locals = [(r, os.path.join(dst_dir, r.rfilename))
                     for r in repo_files]
    to_redownload = process_map(
        check_valid, rf_and_locals, max_workers=8, chunksize=4)
    to_redownload = [d for d in to_redownload if d is not None]

    print(f"Re-downloading {len(to_redownload)} files")

    for src_url, rp in to_redownload:
        dst_path = os.path.join(dst_dir, rp)
        # Delete dst_path first
        if os.path.exists(dst_path):
            os.remove(dst_path)

        # Use curl to download the file
        tok = HfFolder().get_token()
        headers = {"authorization": f"Bearer {tok}", "user-agent": ""}

        subprocess.run(["curl", "-L", "-o", dst_path, src_url,
                        "-H", f"authorization: Bearer {tok}"])


if __name__ == "__main__":
    dst_dir = sys.argv[1]
    main(dst_dir)
