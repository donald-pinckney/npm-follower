# systemd services for npm-follower
These have paths pointing to my home and other directories, so it is best to them it appropriately  
Make sure to add
```bash
systemctl --user import-environment PATH
```

to your `.bashrc` or `.bash_profile`such that systemd imports your PATH env var
