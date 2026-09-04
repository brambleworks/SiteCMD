import importlib.util
import json
from pathlib import Path
import sqlite3
import subprocess
import sys
from types import SimpleNamespace

data = json.load(sys.stdin)
try:
    if data.get("operation") == "public-tests":
        result = subprocess.run(["/usr/bin/python3", "-B", "-m", "unittest", "discover", "-s", "app/api"], capture_output=True, text=True, timeout=5)
        print(json.dumps({"exitCode": result.returncode, "log": result.stdout + result.stderr}))
        sys.exit(0)
    spec = importlib.util.spec_from_file_location("candidate", "/work/" + data["entry"])
    candidate = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(candidate)
    request = SimpleNamespace(args=data["args"])
    if data["operation"] == "lookup":
        connection = sqlite3.connect(":memory:")
        connection.execute("CREATE TABLE users (id INTEGER PRIMARY KEY, name TEXT)")
        connection.executemany("INSERT INTO users VALUES (?, ?)", data["users"])
        result = candidate.lookup(connection, request)
        remaining = connection.execute("SELECT id, name FROM users ORDER BY id").fetchall()
        print(json.dumps({"result": result, "remaining": remaining}))
    else:
        root = Path("/tmp/documents")
        (root / "nested").mkdir(parents=True)
        (root / "readme.txt").write_text(data["text"])
        (root / "nested" / "guide.txt").write_text(data["text"] + " nested")
        Path("/tmp/outside.txt").write_text("outside-root-canary")
        (root / "escape").symlink_to("/tmp/outside.txt")
        print(json.dumps({"result": candidate.download(str(root), request)}))
except Exception as error:
    print(json.dumps({"error": type(error).__name__, "message": str(error)}))
