import sys
from huggingface_hub import HfApi, CommitOperationAdd

def main():
    args = sys.argv[1:]
    token = args[0]
    repo = args[1]

    ops = []
    for i in range(2, len(args), 2):
        local_name = args[i]
        remote_name = args[i + 1]
        print(f"Uploading {local_name} to {remote_name}")
        ops.append(
            CommitOperationAdd(path_in_repo=remote_name, path_or_fileobj=local_name)
        )
    
    api = HfApi(
        token=token
    )
    api.create_commit(
        repo_id=repo,
        operations=ops,
        commit_message="uploading files",
        repo_type="dataset",
    )


if __name__ == "__main__":
    main()
