import avc
import subprocess

subprocess.run(["git", "init"])
a = avc.Avc(data_dir=None, initialize=True, cloned=True)
a.fast_forward()
