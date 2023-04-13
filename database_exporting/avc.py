import sqlite3
import os
import sys
import argparse
import subprocess
import stat
import datetime
import platform
import hashlib
import json
from abc import ABC, abstractmethod
from typing import List, NamedTuple, Optional, Tuple


class ReadSource(NamedTuple):
    local_path: str
    offset: int
    num_bytes: int

    def to_json(self):
        return {
            "local_path": self.local_path,
            "offset": self.offset,
            "num_bytes": self.num_bytes
        }


class VirtualAddOperation(ABC):
    @abstractmethod
    def to_json(self):
        raise NotImplementedError

    @staticmethod
    def from_json(json_dict):
        if json_dict["type"] == "ConcatenatingAddOperation":
            return ConcatenatingAddOperation(json_dict["repo_path"], [ReadSource(**source) for source in json_dict["sources"]])
        elif json_dict["type"] == "DirectAddOperation":
            return DirectAddOperation(json_dict["repo_path"], json_dict["local_path"])
        else:
            raise Exception(f"Unknown type: {json_dict['type']}")


class ConcatenatingAddOperation(VirtualAddOperation):
    def __init__(self, repo_path: str, sources: List[ReadSource]):
        self.repo_path = repo_path
        self.sources = sources

    def __repr__(self) -> str:
        return f"ConcatenatingAddOperation(repo_path={self.repo_path}, sources={self.sources})"

    def to_json(self):
        return {
            "type": "ConcatenatingAddOperation",
            "repo_path": self.repo_path,
            "sources": [source.to_json() for source in self.sources]
        }


class DirectAddOperation(VirtualAddOperation):
    def __init__(self, repo_path: str, local_path: str):
        self.repo_path = repo_path
        self.local_path = local_path

    def __repr__(self) -> str:
        return f"DirectAddOperation(repo_path={self.repo_path}, local_path={self.local_path})"

    def to_json(self):
        return {
            "type": "DirectAddOperation",
            "repo_path": self.repo_path,
            "local_path": self.local_path
        }


class Avc(object):
    def __init__(self, initialize: bool):
        # Find top level directory of git repository at cwd

        try:
            toplevel = subprocess.run(["git", "rev-parse", "--show-toplevel"],
                                      check=True, capture_output=True).stdout
            self.toplevel = toplevel.decode("utf-8").strip()
        except subprocess.CalledProcessError:
            print("Not in a git repository")
            raise

        self.avc_dir = os.path.join(self.toplevel, ".avc")
        self.global_db_path = os.path.join(self.avc_dir, "global_state.db")
        self.local_db_path = os.path.join(self.avc_dir, "local_state.db")
        self.blobs_dir = os.path.join(self.avc_dir, "blobs")
        self.git_operations_path = os.path.join(
            self.avc_dir, "git_operations.json")

        if initialize:
            if os.path.exists(self.avc_dir):
                print(f"{self.avc_dir} directory already exists")
                raise Exception("Directory already exists")

            # Create .avc directory and subdirectories
            os.mkdir(self.avc_dir)
            os.mkdir(self.blobs_dir)
            self.global_db_conn = sqlite3.connect(self.global_db_path)
            self.local_db_conn = sqlite3.connect(self.local_db_path)
            self.global_db_conn.row_factory = sqlite3.Row
            self.local_db_conn.row_factory = sqlite3.Row

            try:
                with open(os.path.join(self.toplevel, ".gitignore"), "r") as f:
                    lines = f.readlines()
                    not_ignored = "/.avc/local_state.db" not in lines
            except FileNotFoundError:
                not_ignored = True

            if not_ignored:
                print("Adding /.avc/local_state.db to .gitignore")
                print("Adding /.avc/git_operations.json to .gitignore")
                with open(os.path.join(self.toplevel, ".gitignore"), "a") as f:
                    f.write("/.avc/local_state.db\n")
                    f.write("/.avc/git_operations.json\n")

            self.initialize_db()

        else:
            if not os.path.exists(self.avc_dir):
                print(f"{self.avc_dir} directory does not exist")
                raise Exception("Directory does not exist")
            if not os.path.exists(self.global_db_path):
                print(f"{self.global_db_path} file does not exist")
                raise Exception("File does not exist")
            if not os.path.exists(self.local_db_path):
                print(f"{self.local_db_path} file does not exist")
                raise Exception("File does not exist")
            if not os.path.exists(self.blobs_dir):
                print(f"{self.blobs_dir} directory does not exist")
                raise Exception("Directory does not exist")

            self.global_db_conn = sqlite3.connect(self.global_db_path)
            self.local_db_conn = sqlite3.connect(self.local_db_path)
            self.global_db_conn.row_factory = sqlite3.Row
            self.local_db_conn.row_factory = sqlite3.Row

    def initialize_db(self):
        self.global_db_conn.executescript("""
            BEGIN;
            CREATE TABLE commits (
                id VARCHAR PRIMARY KEY NOT NULL,
                parent_id VARCHAR
            );
            CREATE INDEX commits_parent_idx ON commits (parent_id);
            CREATE TABLE commit_changes (
                commit_id VARCHAR NOT NULL,
                path VARCHAR NOT NULL,
                type VARCHAR NOT NULL,
                start_offset INTEGER NOT NULL,
                num_bytes INTEGER NOT NULL,
                blob_name VARCHAR NOT NULL,
                blob_offset INTEGER NOT NULL,
                FOREIGN KEY (commit_id) REFERENCES commits (id),
                PRIMARY KEY (commit_id, path)
            );
            CREATE TABLE remote_refs (
                name VARCHAR PRIMARY KEY NOT NULL,
                commit_id VARCHAR
            );
            INSERT INTO remote_refs (name, commit_id) VALUES ("main", NULL);
            COMMIT;
        """)

        self.local_db_conn.executescript("""
            BEGIN;
            CREATE TABLE staged_changes (
                path VARCHAR PRIMARY KEY NOT NULL,
                type VARCHAR NOT NULL,
                start_offset INTEGER NOT NULL,
                num_bytes INTEGER NOT NULL,
                backing_path VARCHAR NOT NULL,
                backing_offset INTEGER NOT NULL
            );
            CREATE TABLE local_refs (
                name VARCHAR PRIMARY KEY NOT NULL,
                commit_id VARCHAR
            );
            INSERT INTO local_refs (name, commit_id) VALUES ("HEAD", NULL);
            COMMIT;
        """)

    def status(self):
        print("All tables:")
        for db in [self.global_db_conn, self.local_db_conn]:
            for row in db.execute("SELECT type, name FROM sqlite_master"):
                if row[0] == "table":
                    print(f"{row[1]}:")
                    for row_inner in db.execute(f"SELECT * FROM {row[1]}"):
                        print(dict(row_inner))
                    print()

    def get_head(self) -> Optional[str]:
        return self.local_db_conn.execute(
            "SELECT commit_id FROM local_refs WHERE name = 'HEAD'").fetchone()[0]

    def get_main(self) -> Optional[str]:
        return self.global_db_conn.execute(
            "SELECT commit_id FROM remote_refs WHERE name = 'main'").fetchone()[0]

    def get_committed_file_size(self, repo_file_path: str) -> Optional[int]:
        commit_id = self.get_head()

        global_cur = self.global_db_conn.cursor()

        while commit_id is not None:
            global_cur.execute(
                "SELECT start_offset, num_bytes FROM commit_changes WHERE commit_id = ? AND path = ?", (commit_id, repo_file_path))
            row = global_cur.fetchone()
            if row is not None:
                return row[0] + row[1]
            commit_id = global_cur.execute(
                "SELECT parent_id FROM commits WHERE id = ?", (commit_id,)).fetchone()[0]

        return None

    def add(self, local_file_path: str, num_bytes: Optional[int]):
        head = self.get_head()
        main = self.get_main()
        if head != main:
            print("Cannot add files while HEAD is not equal to main")
            raise Exception("HEAD is not equal to main")

        repo_file_path = os.path.relpath(local_file_path, self.toplevel)

        local_file_path = os.path.abspath(local_file_path)

        s = os.stat(local_file_path)
        if not stat.S_ISREG(s.st_mode):
            print(f"{local_file_path} is not a regular file")
            raise Exception("Not a regular file")
        file_size = s.st_size
        if num_bytes is None:
            num_bytes = file_size
        else:
            if num_bytes > file_size:
                print(f"{local_file_path} is not {num_bytes} bytes long")
                raise Exception("Not enough bytes")

        local_cur = self.local_db_conn.cursor()

        committed_num_bytes: Optional[int] = self.get_committed_file_size(
            repo_file_path)
        staged_row: Optional[Tuple[str, int, int]] = local_cur.execute(
            "SELECT type, start_offset, num_bytes FROM staged_changes WHERE path = ?", (repo_file_path,)).fetchone()

        if committed_num_bytes is None:
            # Then we are creating this file
            if staged_row is not None:
                assert staged_row[0] == "create"
                assert staged_row[1] == 0
                new_num_bytes = max(staged_row[2], num_bytes)
                local_cur.execute("""
                    UPDATE staged_changes
                    SET num_bytes = ?
                    WHERE path = ?
                """, (new_num_bytes, repo_file_path))
            else:
                local_cur.execute("""
                    INSERT INTO staged_changes (path, type, start_offset, num_bytes, backing_path, backing_offset)
                    VALUES (?, "create", 0, ?, ?, 0)
                """, (repo_file_path, num_bytes, local_file_path))
        else:
            if num_bytes <= committed_num_bytes:
                print(
                    f"Nothing new to add with {num_bytes} bytes: {local_file_path} is already has {committed_num_bytes} committed bytes")
            # We are appending to this file
            if staged_row is not None:
                assert staged_row[0] == "append"
                assert staged_row[1] == committed_num_bytes
                new_num_bytes = max(
                    staged_row[2], num_bytes - committed_num_bytes)
                local_cur.execute("""
                    UPDATE staged_changes
                    SET num_bytes = ?
                    WHERE path = ?
                """, (new_num_bytes, repo_file_path))
            else:
                local_cur.execute("""
                    INSERT INTO staged_changes (path, type, start_offset, num_bytes, backing_path, backing_offset)
                    VALUES (?, "append", ?, ?, ?, ?)
                """, (repo_file_path, committed_num_bytes, num_bytes - committed_num_bytes, local_file_path, committed_num_bytes))

        self.local_db_conn.commit()

    def reset_staged(self):
        self.local_db_conn.executescript("""
            BEGIN;
            DELETE FROM staged_changes;
            COMMIT;
        """)

    def build_git_commit(self, dry_run: bool) -> List[VirtualAddOperation]:
        head = self.get_head()
        main = self.get_main()
        if head != main:
            print("Cannot commit while HEAD is not equal to main")
            raise Exception("HEAD is not equal to main")

        # 1. Get staged changes
        all_staged_changes: list[sqlite3.Row] = self.local_db_conn.execute("""
            SELECT path, type, start_offset, num_bytes, backing_path, backing_offset
            FROM staged_changes
        """).fetchall()

        if len(all_staged_changes) == 0:
            print("No staged changes")
            return []

        # 2. Compute commit ID
        sha = hashlib.sha256()
        parent_id: Optional[str] = head
        if parent_id is None:
            sha.update(b"n/a\n")
        else:
            sha.update(parent_id.encode("utf-8") + b"\n")
        date_str = datetime.datetime.now().isoformat()
        sha.update(date_str.encode("utf-8") + b"\n")
        author_hostname = platform.node()
        sha.update(author_hostname.encode("utf-8") + b"\n")
        changes_str = "\n".join([str(dict(s)) for s in all_staged_changes])
        sha.update(changes_str.encode("utf-8") + b"\n")
        commit_id = sha.hexdigest()

        print(commit_id)

        # 3. Compute Commit Changes
        create_staged_changes = [
            s for s in all_staged_changes if s["type"] == "create"]
        append_staged_changes = [
            s for s in all_staged_changes if s["type"] == "append"]

        blob_id = 0

        def alloc_blob():
            nonlocal blob_id
            blob_name = f"{commit_id[:2]}/{commit_id}-blob-{blob_id}"
            blob_id += 1
            return blob_name

        def get_blob_path(blob_id: str):
            return os.path.join(".avc", "blobs", blob_id)

        required_git_operations: List[VirtualAddOperation] = []

        create_commit_changes = []
        for s in create_staged_changes:
            assert s["start_offset"] == 0
            blob_name = alloc_blob()
            blob_path = get_blob_path(blob_name)
            create_commit_changes.append((
                commit_id,
                s["path"],
                "create",
                0,
                s["num_bytes"],
                blob_name,
                0
            ))
            required_git_operations.append(ConcatenatingAddOperation(blob_path, [ReadSource(
                s["backing_path"], s["backing_offset"], s["num_bytes"])]))

        append_commit_changes = []
        if len(append_staged_changes) > 0:
            append_blob_name = alloc_blob()
            append_blob_path = get_blob_path(append_blob_name)

            read_sources = []

            append_blob_offset = 0
            for s in append_staged_changes:
                append_commit_changes.append((
                    commit_id,
                    s["path"],
                    "append",
                    s["start_offset"],
                    s["num_bytes"],
                    append_blob_name,
                    append_blob_offset
                ))
                read_sources.append(ReadSource(
                    s["backing_path"], s["backing_offset"], s["num_bytes"]))
                append_blob_offset += s["num_bytes"]

            required_git_operations.append(
                ConcatenatingAddOperation(append_blob_path, read_sources))

        if not dry_run:
            # 4. Write changes to global DB
            self.global_db_conn.execute("""
                INSERT INTO commits (id, parent_id)
                VALUES (?, ?)
            """, (commit_id, parent_id))
            self.global_db_conn.executemany("""
                INSERT INTO commit_changes (commit_id, path, type, start_offset, num_bytes, blob_name, blob_offset)
                VALUES (?, ?, ?, ?, ?, ?, ?)
            """, create_commit_changes + append_commit_changes)
            self.global_db_conn.execute("""
                UPDATE remote_refs SET commit_id = ?
                WHERE name = 'main'
            """, (commit_id,))
            self.global_db_conn.commit()
            self.global_db_conn.close()
            self.global_db_conn = sqlite3.connect(self.global_db_path)

            # 5. Update local DB
            self.local_db_conn.execute("""
                UPDATE local_refs SET commit_id = ?
                WHERE name = 'HEAD'
            """, (commit_id,))
            self.local_db_conn.execute("""
                DELETE FROM staged_changes
            """)
            self.local_db_conn.commit()

        required_git_operations.append(DirectAddOperation(
            os.path.join(".avc", "global_state.db"), self.global_db_path))

        with open(self.git_operations_path, "w") as f:
            json.dump([op.to_json()
                      for op in required_git_operations], f, indent=4)

        return required_git_operations

    def abort_last_commit(self):
        commit_id = self.get_head()
        if commit_id is None:
            print("No commits to abort")
            return

        parent_id: Optional[str] = self.global_db_conn.execute("""
            SELECT parent_id FROM commits WHERE id = ?
        """, (commit_id,)).fetchone()[0]

        self.local_db_conn.execute("""
            UPDATE local_refs SET commit_id = ?
            WHERE name = 'HEAD'
        """, (parent_id,))
        self.local_db_conn.commit()

        self.global_db_conn.execute("""
            UPDATE remote_refs SET commit_id = ?
            WHERE name = 'main'
        """, (parent_id,))
        self.global_db_conn.execute("""
            DELETE FROM commit_changes WHERE commit_id = ?
        """, (commit_id,))
        self.global_db_conn.execute("""
            DELETE FROM commits WHERE id = ?
        """, (commit_id,))
        self.global_db_conn.commit()

    def __del__(self):
        if hasattr(self, "local_db_conn"):
            self.local_db_conn.close()
        if hasattr(self, "global_db_conn"):
            self.global_db_conn.close()


def main_init():
    Avc(initialize=True)


def main_status():
    avc = Avc(initialize=False)
    avc.status()


def main_add(path: str, num_bytes: Optional[int] = None):
    avc = Avc(initialize=False)
    avc.add(path, num_bytes)


def main_reset_staged():
    avc = Avc(initialize=False)
    avc.reset_staged()


def main_abort_last_commit():
    avc = Avc(initialize=False)
    avc.abort_last_commit()


def main_build_git_commit(dry_run: bool):
    avc = Avc(initialize=False)
    avc.build_git_commit(dry_run)
    print("You now MUST push the operations in the followeing file to the remote:")
    print(avc.git_operations_path)


def main():
    # Define an argument parser that accepts 4 subcommands: init, status, add, commit.
    # The init command accepts no arguments
    # The status command accepts no arguments
    # The add command accepts a single argument: the path to the file to add
    # The build-git-commit command accepts one argument: dry-run

    parser = argparse.ArgumentParser()
    subparsers = parser.add_subparsers(dest="subcommand")
    subparsers.required = True
    subparsers.add_parser("init")
    subparsers.add_parser("status")
    add_parser = subparsers.add_parser("add")
    add_parser.add_argument("path")
    add_parser.add_argument("--num-bytes", type=int)
    subparsers.add_parser("reset-staged")
    build_git_commit_parser = subparsers.add_parser("build-git-commit")
    build_git_commit_parser.add_argument("--dry-run", action="store_true")
    # subparsers.add_parser("confirm-push")
    subparsers.add_parser("abort-last-commit")
    args = parser.parse_args()
    if args.subcommand == "init":
        main_init()
    elif args.subcommand == "status":
        main_status()
    elif args.subcommand == "add":
        main_add(args.path, args.num_bytes)
    elif args.subcommand == "reset-staged":
        main_reset_staged()
    elif args.subcommand == "build-git-commit":
        main_build_git_commit(args.dry_run)
    # elif args.subcommand == "confirm-push":
    #     main_confirm_push()
    elif args.subcommand == "abort-last-commit":
        main_abort_last_commit()
    else:
        raise Exception("Invalid subcommand")


if __name__ == "__main__":
    main()
