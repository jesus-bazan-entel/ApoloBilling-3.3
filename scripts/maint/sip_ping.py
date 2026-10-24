#!/usr/bin/env python3
"""Send one SIP OPTIONS over UDP and print the status line of the reply.

Usage: sip_ping.py <host> <port> [source_ip]
Exit 0 on any SIP reply (the server is alive and parsing), 1 on timeout.
Used by restart_kamailio_safe.sh / restart_freeswitch_safe.sh.
"""
import random
import socket
import sys

host, port = sys.argv[1], int(sys.argv[2])
src = sys.argv[3] if len(sys.argv) > 3 else host

s = socket.socket(socket.AF_INET, socket.SOCK_DGRAM)
s.bind((src, 0))
s.settimeout(2)
lip, lp = s.getsockname()
branch = "z9hG4bK%x" % random.getrandbits(48)
msg = (
    f"OPTIONS sip:ping@{host}:{port} SIP/2.0\r\n"
    f"Via: SIP/2.0/UDP {lip}:{lp};branch={branch};rport\r\n"
    "Max-Forwards: 70\r\n"
    f"From: <sip:ping@{lip}>;tag={random.getrandbits(32)}\r\n"
    f"To: <sip:ping@{host}>\r\n"
    f"Call-ID: {random.getrandbits(64)}@apolo-ping\r\n"
    "CSeq: 1 OPTIONS\r\n"
    "Content-Length: 0\r\n\r\n"
)
s.sendto(msg.encode(), (host, port))
try:
    print(s.recv(4096).decode(errors="replace").split("\r\n")[0])
    sys.exit(0)
except socket.timeout:
    print("TIMEOUT")
    sys.exit(1)
