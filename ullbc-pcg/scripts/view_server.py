#!/usr/bin/env python3
"""Local-only source submission and per-run results for the ULLBC viewer."""
import argparse
import html
import json
import mimetypes
import shutil
import subprocess
import tempfile
import threading
import uuid
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from pathlib import Path
from urllib.parse import parse_qs, urlsplit


class ViewerServer(ThreadingHTTPServer):
    daemon_threads = True

    def __init__(self, port, repo, charon, analyzer):
        self.repo = repo
        self.charon = charon
        self.analyzer = analyzer
        self.viewer = repo / 'ullbc-pcg/visualization'
        self.work = repo / 'target/pcg-viewer/ullbc'
        self.work.mkdir(parents=True, exist_ok=True)
        self.analysis_lock = threading.Lock()
        super().__init__(('127.0.0.1', port), Handler)

    def analyze(self, code, function):
        run_id = uuid.uuid4().hex
        output = self.viewer / 'data/runs' / run_id
        with self.analysis_lock, tempfile.TemporaryDirectory(dir=self.work) as work:
            source = Path(work) / 'input.rs'
            extracted = Path(work) / 'input.ullbc'
            source.write_text(code, encoding='utf-8')
            commands = [
                [str(self.charon), 'rustc', '--ullbc', '--export-borrowck', '--mir', 'promoted', '--sysroot', 'default',
                 '--dest-file', str(extracted), '--', '--crate-type', 'lib', '--crate-name',
                 'ullbc_pcg_input', str(source)],
                [str(self.analyzer), str(extracted), '--output', str(output)],
            ]
            if function:
                commands[1].extend(['--function', function])
            try:
                for command in commands:
                    result = subprocess.run(command, cwd=self.repo, text=True,
                                            stdout=subprocess.PIPE, stderr=subprocess.STDOUT,
                                            timeout=120)
                    if result.returncode:
                        raise ValueError(result.stdout)
            except (ValueError, subprocess.TimeoutExpired, OSError):
                shutil.rmtree(output, ignore_errors=True)
                raise
        return '/visualization/?datasrc=/data/runs/' + run_id


class Handler(BaseHTTPRequestHandler):
    def reply(self, status, body, content_type='text/plain; charset=utf-8'):
        if isinstance(body, str):
            body = body.encode('utf-8')
        self.send_response(status)
        self.send_header('Content-Type', content_type)
        self.send_header('Content-Length', str(len(body)))
        self.send_header('Cache-Control', 'no-store')
        self.send_header('X-Content-Type-Options', 'nosniff')
        self.end_headers()
        self.wfile.write(body)

    def local_request(self):
        port = self.server.server_address[1]
        hosts = {f'127.0.0.1:{port}', f'localhost:{port}'}
        origin = self.headers.get('Origin')
        return (self.headers.get('Host') in hosts and
                (origin is None or origin in {'http://' + host for host in hosts}))

    def do_GET(self):
        if not self.local_request():
            return self.reply(403, 'Local requests only.')
        path = urlsplit(self.path).path
        if path == '/health':
            return self.reply(200, json.dumps({'service': 'ullbc-pcg-view', 'version': 1,
                                               'repo': str(self.server.repo)}), 'application/json')
        if path == '/':
            return self.reply(200, (self.server.repo / 'ullbc-pcg/scripts/view_home.html').read_bytes(),
                              'text/html; charset=utf-8')
        if path == '/visualization/':
            target = self.server.viewer / 'index.html'
        elif path.startswith('/visualization/dist/'):
            target = self.server.viewer / path.removeprefix('/visualization/')
        elif path.startswith('/data/runs/'):
            target = self.server.viewer / path.lstrip('/')
        else:
            return self.reply(404, 'Not found')
        # Only public viewer assets and completed results are exposed.
        target = target.resolve()
        allowed = [self.server.viewer / 'dist', self.server.viewer / 'data/runs']
        if target != self.server.viewer / 'index.html' and not any(
                root.resolve() in target.parents for root in allowed):
            return self.reply(404, 'Not found')
        if not target.is_file():
            return self.reply(404, 'Not found')
        return self.reply(200, target.read_bytes(), mimetypes.guess_type(target)[0] or 'text/plain')

    def do_POST(self):
        if not self.local_request():
            return self.reply(403, 'Local requests only.')
        if self.path != '/upload':
            return self.reply(404, 'Not found')
        if self.headers.get('Content-Type', '').split(';')[0] != 'application/x-www-form-urlencoded':
            return self.reply(415, 'Expected form data')
        try:
            length = int(self.headers.get('Content-Length', '0'))
            if not 0 < length <= 4 * 1024 * 1024:
                return self.reply(413, 'Source submission is empty or too large (4 MiB form limit).')
            fields = parse_qs(self.rfile.read(length).decode('utf-8'), max_num_fields=8)
            code = fields.get('code', [''])[0]
            function = fields.get('function', [''])[0].strip()
            if not code.strip():
                raise ValueError('请输入或上传 Rust 代码。')
            location = self.server.analyze(code, function)
        except (ValueError, UnicodeError, subprocess.TimeoutExpired, OSError) as error:
            return self.reply(422, '<meta charset="utf-8"><h1>分析失败</h1><pre>' +
                              html.escape(str(error)) + '</pre><p>返回上一页修改代码或指定函数名。</p>',
                              'text/html; charset=utf-8')
        self.send_response(303)
        self.send_header('Location', location)
        self.send_header('Content-Length', '0')
        self.end_headers()


if __name__ == '__main__':
    parser = argparse.ArgumentParser()
    parser.add_argument('--port', type=int, default=5180)
    parser.add_argument('--repo', type=Path, required=True)
    parser.add_argument('--charon', type=Path, required=True)
    parser.add_argument('--analyzer', type=Path, required=True)
    args = parser.parse_args()
    server = ViewerServer(args.port, args.repo.resolve(), args.charon.resolve(), args.analyzer.resolve())
    print(f'ULLBC-PCG listening on http://127.0.0.1:{args.port}/', flush=True)
    try:
        server.serve_forever()
    except KeyboardInterrupt:
        pass
    finally:
        server.server_close()
