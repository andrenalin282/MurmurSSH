"""E2E test of MurmurSSH AI access: real app (isolated HOME) + MCP bridge + sshd container.

Usage: docker run -d --name murmur-sshd -p 127.0.0.1:2222:2222 -e PUID=$(id -u) -e PGID=$(id -g) \
         -e USER_NAME=murmur -e PASSWORD_ACCESS=false lscr.io/linuxserver/openssh-server
       MURMURSSH_BIN=path/to/murmurssh python3 -I scripts/e2e_ai_access.py /tmp/murmur-e2e
Opens a MurmurSSH window (do NOT click the AI dialogs: the test proves that nothing
happens without a click). Takes ~2.5 min.
"""
import base64, hashlib, json, os, shutil, stat, subprocess, sys, threading, time

BIN = os.environ.get("MURMURSSH_BIN", "src-tauri/target/release/murmurssh")
S = sys.argv[1]
HOME = f"{S}/home"
CFG = f"{HOME}/.config/murmurssh"
LOCAL = f"{S}/local"
SOCK = f"{os.environ['XDG_RUNTIME_DIR']}/murmurssh/ai.sock"
results = []

def check(name, cond, info=""):
    results.append((name, bool(cond)))
    print(("PASS " if cond else "FAIL ") + name + ("" if cond else f"  -> {info}"), flush=True)

def sh(cmd):
    return subprocess.run(cmd, shell=True, capture_output=True, text=True)

def dexec(cmd):
    return sh(f"docker exec murmur-sshd sh -c {json.dumps(cmd)}")

# ---------- setup ----------
if os.path.exists(S) and not os.path.isfile(f"{S}/home/.config/murmurssh/ai_access.json"):
    sys.exit(f"refusing to wipe {S}: not a previous e2e run directory")
shutil.rmtree(S, ignore_errors=True)
os.makedirs(f"{CFG}/profiles"); os.makedirs(f"{LOCAL}/site/sub")
open(f"{LOCAL}/site/index.html", "w").write("<h1>hi</h1>")
open(f"{LOCAL}/site/sub/a.txt", "w").write("aaa")
open(f"{LOCAL}/single.txt", "w").write("single")
sh(f"ssh-keygen -q -t ed25519 -N '' -f {S}/key")
pub = open(f"{S}/key.pub").read().strip()
dexec(f"mkdir -p /config/.ssh /config/ai /config/secret && echo '{pub}' >> /config/.ssh/authorized_keys && chmod 600 /config/.ssh/authorized_keys && echo topsecret > /config/secret/s.txt && echo hello > /config/ai/hello.txt && ln -sfn /config/secret /config/ai/esc && chown -R 1000:1000 /config/ai /config/secret /config/.ssh")

def fp(keytype):
    out = [l for l in sh(f"ssh-keyscan -t {keytype} -p 2222 127.0.0.1 2>/dev/null").stdout.splitlines() if l and not l.startswith("#")][0].split()
    return ":".join(f"{b:02x}" for b in hashlib.sha256(base64.b64decode(out[2])).digest())

profile = {"id": "e2e", "name": "E2E Test", "host": "127.0.0.1", "port": 2222, "username": "murmur",
           "auth_type": "key", "key_path": f"{S}/key", "default_remote_path": "/config/ai",
           "editor_command": None, "upload_mode": "auto"}
json.dump(profile, open(f"{CFG}/profiles/e2e.json", "w"))
json.dump({**profile, "id": "other", "name": "Other"}, open(f"{CFG}/profiles/other.json", "w"))

def tok(secret, **kw):
    caps = {"read": False, "download": False, "upload": False, "write": False, "delete": False, "exec": False, "confirm_writes": False}
    caps.update(kw.pop("caps", {}))
    t = {"id": hashlib.md5(secret.encode()).hexdigest()[:16], "name": kw.pop("name"), "hash": hashlib.sha256(secret.encode()).hexdigest(),
         "prefix": secret[:11], "profiles": ["e2e"], "caps": caps, "remote_root": "/config/ai", "local_root": LOCAL,
         "created_at": int(time.time()), "expires_at": None, "last_used_at": None}
    t.update(kw); return t

A, B, C, D = ("mssh_" + ch * 43 for ch in "ABCD")
store = {"enabled": True, "tokens": [
    tok(A, name="rw", caps={"read": True, "download": True, "upload": True, "write": True}),
    tok(B, name="danger", caps={"read": True, "delete": True, "exec": True}),
    tok(C, name="expired", caps={"read": True}, expires_at=int(time.time()) - 10),
    tok(D, name="sshdirect", caps={"ssh_direct": True})]}
def write_store():
    json.dump(store, open(f"{CFG}/ai_access.json", "w")); os.chmod(f"{CFG}/ai_access.json", 0o600)

def set_known(keytype):
    open(f"{CFG}/known_hosts", "w").write(f"127.0.0.1:2222 {fp(keytype)}\n")

write_store(); set_known("ecdsa")

# ---------- app ----------
env = {**os.environ, "HOME": HOME}
if os.path.exists(SOCK): os.remove(SOCK)  # stale socket from a killed previous run
app = subprocess.Popen([BIN], env=env, stdout=open(f"{S}/app.log", "w"), stderr=subprocess.STDOUT)
for _ in range(100):
    if os.path.exists(SOCK): break
    time.sleep(0.2)
check("socket created by app", os.path.exists(SOCK), open(f"{S}/app.log").read()[-500:])
if os.path.exists(SOCK):
    check("socket mode 0600", stat.S_IMODE(os.stat(SOCK).st_mode) == 0o600, oct(os.stat(SOCK).st_mode))
    check("socket dir mode 0700", stat.S_IMODE(os.stat(os.path.dirname(SOCK)).st_mode) == 0o700)

class Bridge:
    def __init__(self, token):
        e = {**env}
        e.pop("MURMURSSH_TOKEN", None)
        if token: e["MURMURSSH_TOKEN"] = token
        self.p = subprocess.Popen([BIN, "--mcp"], env=e, stdin=subprocess.PIPE, stdout=subprocess.PIPE, text=True)
        self.n = 0
    def rpc(self, method, params=None):
        self.n += 1
        self.p.stdin.write(json.dumps({"jsonrpc": "2.0", "id": self.n, "method": method, "params": params or {}}) + "\n"); self.p.stdin.flush()
        return json.loads(self.p.stdout.readline())
    def call(self, name, **args):
        r = self.rpc("tools/call", {"name": name, "arguments": args})["result"]
        return r["isError"], r["content"][0]["text"]
    def close(self):
        self.p.stdin.close(); self.p.wait(5)

b = Bridge(A)
init = b.rpc("initialize", {"protocolVersion": "2025-06-18", "capabilities": {}, "clientInfo": {"name": "e2e", "version": "1"}})
check("initialize", init["result"]["serverInfo"]["name"] == "murmurssh")
b.p.stdin.write(json.dumps({"jsonrpc": "2.0", "method": "notifications/initialized"}) + "\n")
check("tools/list = 10", len(b.rpc("tools/list")["result"]["tools"]) == 10)

err, txt = b.call("murmur_list_dir", profile="e2e", path="/config/ai")
if err and "Host key" in txt:
    set_known("ed25519"); err, txt = b.call("murmur_list_dir", profile="e2e", path="/config/ai")
check("list_dir in root", not err and "hello.txt" in txt, txt)

err, txt = b.call("murmur_list_profiles")
check("list_profiles only allowed profile", not err and '"e2e"' in txt and '"other"' not in txt, txt)
err, txt = b.call("murmur_list_dir", profile="other", path="/config/ai")
check("profile not on token rejected", err and "not available" in txt, txt)
err, txt = b.call("murmur_list_dir", profile="e2e", path="/config")
check("outside remote root rejected", err and "outside" in txt, txt)
err, txt = b.call("murmur_list_dir", profile="e2e", path="/config/ai/../secret")
check("dotdot traversal rejected", err and "outside" in txt, txt)
err, txt = b.call("murmur_read_file", profile="e2e", path="/config/ai/esc/s.txt")
check("remote symlink escape rejected", err and "outside" in txt and "topsecret" not in txt, txt)
err, txt = b.call("murmur_read_file", profile="e2e", path="/config/ai/hello.txt")
check("read_file", not err and "hello" in txt, txt)

err, txt = b.call("murmur_upload", profile="e2e", local_path="site", remote_path="/config/ai/site")
up = dexec("cat /config/ai/site/index.html /config/ai/site/sub/a.txt").stdout
check("upload folder", not err and "<h1>hi</h1>" in up and "aaa" in up, txt + up)
err, txt = b.call("murmur_upload", profile="e2e", local_path="single.txt", remote_path="/config/ai/site/index.html")
check("upload onto existing without overwrite rejected", err and "exists" in txt, txt)
err, txt = b.call("murmur_upload", profile="e2e", local_path="../key", remote_path="/config/ai/k")
check("local path outside local root rejected", err and "outside" in txt, txt)
os.symlink(f"{S}", f"{LOCAL}/lnk")
err, txt = b.call("murmur_upload", profile="e2e", local_path="lnk/key", remote_path="/config/ai/k")
check("local symlink escape rejected", err and "outside" in txt, txt)

err, txt = b.call("murmur_download", profile="e2e", remote_path="/config/ai/hello.txt", local_path="dl.txt")
check("download file", not err and open(f"{LOCAL}/dl.txt").read().strip() == "hello", txt)
err, txt = b.call("murmur_download", profile="e2e", remote_path="/config/ai/hello.txt", local_path="dl.txt")
check("download onto existing without overwrite rejected", err and "exists" in txt, txt)
err, txt = b.call("murmur_download", profile="e2e", remote_path="/config/ai/site", local_path="site_back")
check("download folder", not err and os.path.exists(f"{LOCAL}/site_back/sub/a.txt"), txt)

# --- round 2: review fixes ---
dexec("ln -sfn /config/secret /config/ai/site/leak && chown -h 1000:1000 /config/ai/site/leak")
err, txt = b.call("murmur_download", profile="e2e", remote_path="/config/ai/site", local_path="site_back2")
check("folder download skips remote symlink (no secret)", not err and not os.path.exists(f"{LOCAL}/site_back2/leak") and "leak" in txt, txt)
os.makedirs(f"{LOCAL}/proj/.git"); open(f"{LOCAL}/proj/.git/config", "w").write("x"); open(f"{LOCAL}/proj/ok.txt", "w").write("ok")
os.symlink(f"{S}", f"{LOCAL}/proj/out")
err, txt = b.call("murmur_upload", profile="e2e", local_path="proj", remote_path="/config/ai/proj")
ls = dexec("find /config/ai/proj").stdout
check("folder upload skips local symlink and .git", not err and "ok.txt" in ls and ".git" not in ls and "out" not in ls and "key" not in ls, txt + ls)
err, txt = b.call("murmur_upload", profile="e2e", local_path="proj", remote_path="/config/ai/proj")
check("folder upload onto existing rejected", err and "exists" in txt, txt)
err, txt = b.call("murmur_download", profile="e2e", remote_path="/config/ai/hello.txt", local_path="proj/.git/hooks-x")
check("local write into .git rejected", err and "protected" in txt, txt)
err, txt = b.call("murmur_list_dir", profile="e2e", path="/config/ai/‮site")
check("bidi char in path rejected", err and "control" in txt, txt)

err, txt = b.call("murmur_mkdir", profile="e2e", path="/config/ai/newdir")
check("mkdir", not err and dexec("test -d /config/ai/newdir && echo y").stdout.strip() == "y", txt)
err, txt = b.call("murmur_rename", profile="e2e", **{"from": "/config/ai/newdir", "to": "/config/ai/renamed"})
check("rename", not err and dexec("test -d /config/ai/renamed && echo y").stdout.strip() == "y", txt)
err, txt = b.call("murmur_delete", profile="e2e", path="/config/ai/renamed")
check("delete without delete permission rejected", err and "permission" in txt, txt)
err, txt = b.call("murmur_exec", profile="e2e", command="id")
check("exec without exec permission rejected", err and "permission" in txt, txt)
err, txt = b.call("murmur_ssh_command", profile="e2e")
check("ssh_command without ssh_direct rejected", err and "permission" in txt, txt)
err, txt = b.call("murmur_list_profiles")
check("list_profiles hides key_path without ssh_direct", not err and "key_path" not in txt, txt)
b.close()

# ssh_direct: connection details + own ssh + exec without a click (must NOT wait ~120 s).
x = Bridge(D); x.rpc("initialize", {"protocolVersion": "2025-06-18"})
err, txt = x.call("murmur_list_profiles")
check("ssh_direct: list_profiles shows key_path + auth_type", not err and f"{S}/key" in txt and '"auth_type": "key"' in txt, txt)
err, txt = x.call("murmur_ssh_command", profile="e2e")
argv = json.loads(txt).get("argv", []) if not err else []
check("ssh_direct: ssh_command pins host key, BatchMode", argv and "BatchMode=yes" in argv and "StrictHostKeyChecking=yes" in argv, txt)
r = subprocess.run(argv + ["echo", "direct-ok"], capture_output=True, text=True, timeout=30) if argv else None
check("ssh_direct: returned ssh command connects", r and r.stdout.strip() == "direct-ok", r and r.stderr)
t0 = time.time(); err, txt = x.call("murmur_exec", profile="e2e", command="echo exec-ok")
check("ssh_direct: exec runs without click", not err and "exec-ok" in txt and time.time() - t0 < 30, txt)
err, txt = x.call("murmur_delete", profile="e2e", path="/config/ai/hello.txt")
check("ssh_direct does not grant delete", err and "permission" in txt, txt)
x.close()

for label, token, expect in [("no token", None, "MURMURSSH_TOKEN"), ("wrong token", "mssh_" + "Z" * 43, "Invalid token"), ("expired token", C, "expired")]:
    x = Bridge(token); x.rpc("initialize", {"protocolVersion": "2025-06-18"})
    err, txt = x.call("murmur_list_dir", profile="e2e", path="/config/ai")
    check(f"{label} rejected", err and expect in txt, txt); x.close()

x = Bridge(B); x.rpc("initialize", {"protocolVersion": "2025-06-18"})
err, txt = x.call("murmur_exec", profile="e2e", command="echo a\ntouch /config/ai/pwned2")
check("multi-line exec rejected before dialog", err and "one line" in txt, txt)
err, txt = x.call("murmur_delete", profile="e2e", path="/config/ai")
check("delete of remote root refused", err and "Refusing" in txt, txt); x.close()

# Destructive actions with permission but NO human click -> must be denied, nothing changes.
out = {}
def run(label, **kw):
    x = Bridge(B); x.rpc("initialize", {"protocolVersion": "2025-06-18"})
    t0 = time.time(); out[label] = (*x.call(**kw), time.time() - t0); x.close()
th = [threading.Thread(target=run, args=("delete",), kwargs={"name": "murmur_delete", "profile": "e2e", "path": "/config/ai/hello.txt"}),
      threading.Thread(target=run, args=("exec",), kwargs={"name": "murmur_exec", "profile": "e2e", "command": "touch /config/ai/pwned"})]
print("waiting ~120 s for unanswered confirmations ...", flush=True)
for t in th: t.start()
for t in th: t.join(200)
d, e = out.get("delete"), out.get("exec")
check("delete without click denied, file kept", d and d[0] and "denied" in d[1].lower() and dexec("cat /config/ai/hello.txt").stdout.strip() == "hello", d)
check("exec without click denied, not run", e and e[0] and "denied" in e[1].lower() and dexec("test -e /config/ai/pwned || echo none").stdout.strip() == "none", e)

# An AI with shell access editing ai_access.json at runtime must gain nothing:
# the app only trusts what it loaded at start or changed via its GUI.
store["tokens"][0]["caps"]["delete"] = True; store["tokens"][0]["caps"]["exec"] = True; write_store()
x = Bridge(A); x.rpc("initialize", {"protocolVersion": "2025-06-18"})
err, txt = x.call("murmur_exec", profile="e2e", command="id")
check("runtime edit of ai_access.json ignored", err and "permission" in txt, txt); x.close()

app.terminate(); app.wait(10)
audit = f"{CFG}/ai_audit.jsonl"
lines = [json.loads(l) for l in open(audit)] if os.path.exists(audit) else []
check("audit log written, 0600", lines and stat.S_IMODE(os.stat(audit).st_mode) == 0o600)
check("audit has denied confirmations", any(l.get("confirmed") is False for l in lines))
check("audit contains no secret", not any(s in open(audit).read() for s in (A, B, C, D)))
print(f"\n{sum(ok for _, ok in results)}/{len(results)} passed")
