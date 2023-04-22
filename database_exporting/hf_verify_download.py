import shutil
from huggingface_hub import HfApi, hf_hub_url, HfFolder
import sys
import os
import subprocess

REPO_ID = "nuprl/npm-follower-data"
REVISION = "v1.0.0-apr-17-2023"
dst_dir = sys.argv[1]

api = HfApi()
data_info = api.dataset_info(
    repo_id=REPO_ID, revision=REVISION, files_metadata=True)
repo_files = data_info.siblings
print(repo_files)

to_redownload = []
for rf in repo_files:
    if rf.lfs is not None:
        assert 'size' in rf.lfs
        assert rf.lfs['size'] == rf.size
    num_bytes = rf.size
    dst_path = os.path.join(dst_dir, rf.rfilename)
    if not os.path.exists(dst_path):
        print(f"File {dst_path} does not exist")
        src_url = hf_hub_url(REPO_ID, filename=rf.rfilename,
                             repo_type="dataset", revision=REVISION)
        to_redownload.append((src_url, rf.rfilename))
        continue
    disk_bytes = os.path.getsize(dst_path)
    if num_bytes != disk_bytes:
        print(
            f"Incorrect number of bytes downloaded for {dst_path}. Should be {num_bytes} bytes instead of {disk_bytes}")
        src_url = hf_hub_url(REPO_ID, filename=rf.rfilename,
                             repo_type="dataset", revision=REVISION)
        to_redownload.append((src_url, rf.rfilename))

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
