import avc
import subprocess
import sys
import os

dst_dir = sys.argv[1]

os.chdir(dst_dir)

subprocess.run(["git", "init"])
a = avc.Avc(data_dir=None, initialize=True, cloned=True)
a.fast_forward()
