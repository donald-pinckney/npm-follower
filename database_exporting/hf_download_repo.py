import shutil
from huggingface_hub import snapshot_download
import sys
import os

MAX_WORKERS = 32

dst_dir = sys.argv[1]
tmp_cache_dir = "__tmp_hf_cache/"

snapshot_download(repo_id="nuprl/npm-follower-data", repo_type="dataset", revision="v1.0.0-apr-17-2023",
                  local_dir=dst_dir, local_dir_use_symlinks=True, cache_dir=tmp_cache_dir, max_workers=MAX_WORKERS)


for root, dirs, files in os.walk(dst_dir):
    for d in dirs:
        dp = os.path.join(root, d)
        assert not os.path.islink(dp)

    for f in files:
        fp = os.path.join(root, f)
        if os.path.islink(fp):
            target = os.readlink(fp)
            os.unlink(fp)
            shutil.move(target, fp)

shutil.rmtree(tmp_cache_dir)
