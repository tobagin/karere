"""Small loopback-only CDP client; never logs protocol messages or page contents."""
import base64
import hashlib
import json
import os
import socket
import struct
from urllib.parse import urlparse


class Client:
    def __init__(self, url, *, port=9333):
        if not 1024 <= port <= 65535:
            raise ValueError("Invalid local diagnostic port")
        parsed = urlparse(url)
        if parsed.scheme != 'ws' or parsed.hostname != '127.0.0.1' or parsed.port != port:
            raise ValueError('CDP endpoint must be the local diagnostic port')
        self.socket = socket.create_connection(('127.0.0.1', port), timeout=35)
        self.buffer = b''
        self.sequence = 0
        key = base64.b64encode(os.urandom(16)).decode()
        request = (f'GET {parsed.path} HTTP/1.1\r\nHost: 127.0.0.1:{port}\r\n'
                   f'Upgrade: websocket\r\nConnection: Upgrade\r\n'
                   f'Sec-WebSocket-Key: {key}\r\nSec-WebSocket-Version: 13\r\n\r\n')
        self.socket.sendall(request.encode())
        while b'\r\n\r\n' not in self.buffer:
            chunk = self.socket.recv(4096)
            if not chunk:
                raise ConnectionError('CDP handshake closed')
            self.buffer += chunk
        header, self.buffer = self.buffer.split(b'\r\n\r\n', 1)
        expected = base64.b64encode(hashlib.sha1((key + '258EAFA5-E914-47DA-95CA-C5AB0DC85B11').encode()).digest())
        if not header.startswith(b'HTTP/1.1 101 ') or expected.lower() not in header.lower():
            self.socket.close()
            raise ConnectionError('CDP websocket handshake failed')

    def read(self, count):
        while len(self.buffer) < count:
            chunk = self.socket.recv(max(4096, count - len(self.buffer)))
            if not chunk:
                raise ConnectionError('CDP socket closed')
            self.buffer += chunk
        result, self.buffer = self.buffer[:count], self.buffer[count:]
        return result

    def send(self, payload, opcode=1):
        length = len(payload)
        header = bytes([0x80 | opcode])
        if length < 126:
            header += bytes([0x80 | length])
        elif length < 65536:
            header += b'\xfe' + struct.pack('!H', length)
        else:
            header += b'\xff' + struct.pack('!Q', length)
        mask = os.urandom(4)
        self.socket.sendall(header + mask + bytes(value ^ mask[i % 4] for i, value in enumerate(payload)))

    def receive(self):
        parts = []
        while True:
            first, second = self.read(2)
            length = second & 127
            if length == 126:
                length = struct.unpack('!H', self.read(2))[0]
            elif length == 127:
                length = struct.unpack('!Q', self.read(8))[0]
            if second & 128 or length > 1000000:
                raise ValueError('Unexpected CDP frame')
            data = self.read(length)
            opcode = first & 15
            if opcode == 8:
                raise ConnectionError('CDP socket closed')
            if opcode == 9:
                self.send(data, 10)
                continue
            if opcode == 10:
                continue
            parts.append(data)
            if first & 128:
                return json.loads(b''.join(parts))

    def call(self, method, params):
        self.sequence += 1
        self.send(json.dumps({'id': self.sequence, 'method': method, 'params': params}).encode())
        while True:
            reply = self.receive()
            if reply.get('id') == self.sequence:
                if 'error' in reply:
                    raise RuntimeError('CDP method rejected')
                return reply['result']

    def evaluate(self, expression, await_promise=False):
        response = self.call('Runtime.evaluate', {'expression': expression,
                             'returnByValue': True, 'awaitPromise': await_promise})
        if 'exceptionDetails' in response:
            raise RuntimeError('Diagnostic page script failed')
        return response.get('result', {}).get('value')

    def close(self):
        self.socket.close()
