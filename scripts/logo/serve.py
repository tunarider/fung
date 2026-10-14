"""Serves the logo folder and saves POSTed PNGs to <root>/png/<name>.png
(local preview helper, bound to 127.0.0.1)."""
import http.server, os, re, sys, base64
root = sys.argv[1]
os.makedirs(os.path.join(root, "png"), exist_ok=True)

class H(http.server.SimpleHTTPRequestHandler):
    def __init__(self, *a, **k):
        super().__init__(*a, directory=root, **k)
    def do_POST(self):
        m = re.fullmatch(r"/save/([a-z0-9-]+)\.png", self.path)
        if not m:
            self.send_error(404); return
        data = self.rfile.read(int(self.headers["Content-Length"]))
        png = base64.b64decode(data.split(b",", 1)[1])
        open(os.path.join(root, "png", m.group(1) + ".png"), "wb").write(png)
        self.send_response(204); self.end_headers()

http.server.ThreadingHTTPServer(("127.0.0.1", 8765), H).serve_forever()
