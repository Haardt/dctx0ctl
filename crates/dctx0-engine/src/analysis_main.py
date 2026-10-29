# The Rust parent owns this adapter; orphaning cancels the active process group.
import threading
parent_pid = os.getppid()

def parent_watch():
    while os.getppid() == parent_pid:
        time.sleep(0.1)
    os.kill(os.getpid(), signal.SIGTERM)

signal.signal(signal.SIGTERM, lambda *_: sys.exit(130))
threading.Thread(target=parent_watch, daemon=True).start()
os.environ["RUSTUP_AUTO_INSTALL"] = "0"
options = json.loads(sys.argv[1])
print(json.dumps(materialize(options) if options['action'] == 'materialize' else
                 research_diff(options) if options['action'] == 'diff' else
                 analysis_adapter(options)))
