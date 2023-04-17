import avc
import sys
import os


def main():
    a = avc.Avc(data_dir=None, initialize=False)
    blobs_dir = a.data_toplevel

    print(blobs_dir)

    # Loop over files in blobs_dir with .bin extension:
    for file in os.listdir(blobs_dir):
        if file.endswith(".bin"):
            a.add(file, None)
            # avc.add(file)


if __name__ == "__main__":
    main()
