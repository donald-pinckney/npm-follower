import avc
import sys
import os


def main():
    if len(sys.argv) < 2:
        print("Usage: python avc_add_all_blob_files.py <path_to_blob_dir>")
        return

    blobs_dir = sys.argv[1]
    a = avc.Avc(data_dir=blobs_dir, initialize=False)

    # Loop over files in blobs_dir with .bin extension:
    for file in os.listdir(blobs_dir):
        if file.endswith(".bin"):
            a.add(file, None)
            # avc.add(file)

    print(blobs_dir)


if __name__ == "__main__":
    main()
