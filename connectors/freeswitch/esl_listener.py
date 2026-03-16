
import asyncio
import json
import logging
import aiohttp
from datetime import datetime

# Configuration
ESL_HOST = "127.0.0.1"
ESL_PORT = 8021
ESL_PASSWORD = "ClueCon"
BACKEND_URL = "http://localhost:8000/api"

logging.basicConfig(level=logging.INFO, format='%(asctime)s - %(name)s - %(levelname)s - %(message)s')
logger = logging.getLogger("ESLListener")

class ESLClient:
    def __init__(self):
        self.reader = None
        self.writer = None
        self.authenticated = False

    async def connect(self):
        try:
            self.reader, self.writer = await asyncio.open_connection(ESL_HOST, ESL_PORT)
            logger.info(f"Connected to Freeswitch at {ESL_HOST}:{ESL_PORT}")
            await self.authenticate()
        except Exception as e:
            logger.error(f"Connection failed: {e}")
            await asyncio.sleep(5)
            await self.connect()

    async def authenticate(self):
        # Read header
        while True:
            line = await self.reader.readline()
            if not line or line == b"\n":
                break
            if b"Content-Type: auth/request" in line:
                self.writer.write(f"auth {ESL_PASSWORD}\n\n".encode())
                await self.writer.drain()
                
                # Check response
                response = await self.read_response()
                if "Reply-Text: +OK accepted" in response:
                    logger.info("Authenticated successfully")
                    self.authenticated = True
                    await self.subscribe()
                else:
                    logger.error("Authentication failed")
                    self.writer.close()

    async def read_response(self):
        response = ""
        while True:
            line = await self.reader.readline()
            response += line.decode()
            if line == b"\n":
                break
        return response

    async def subscribe(self):
        cmd = "events plain CHANNEL_CREATE CHANNEL_ANSWER CHANNEL_HANGUP\n\n"
        self.writer.write(cmd.encode())
        await self.writer.drain()
        logger.info("Subscribed to events")
        await self.listen()

    async def listen(self):
        try:
            while True:
                # Read headers
                headers = {}
                content_length = 0
                while True:
                    line = await self.reader.readline()
                    line_str = line.decode().strip()
                    if not line_str:
                        break
                    if ":" in line_str:
                        key, value = line_str.split(":", 1)
                        headers[key.strip()] = value.strip()
                
                if "Content-Length" in headers:
                    content_length = int(headers["Content-Length"])
                    body_data = await self.reader.read(content_length)
                    body_str = body_data.decode()
                    
                    # Parse body as event (it is key-value pairs)
                    event_data = self.parse_event_body(body_str)
                    await self.process_event(event_data)
                    
        except Exception as e:
            logger.error(f"Error in listener loop: {e}")
            self.writer.close()
            await self.connect()

    def parse_event_body(self, body):
        data = {}
        for line in body.split("\n"):
            if ":" in line:
                key, value = line.split(":", 1)
                data[key.strip()] = value.strip()
        # Decode some common URL encoded fields if needed
        import urllib.parse
        for k, v in data.items():
            if "%" in v:
                try:
                    data[k] = urllib.parse.unquote(v)
                except:
                    pass
        return data

    async def process_event(self, event):
        event_name = event.get("Event-Name")
        uuid = event.get("Unique-ID")
        logger.info(f"Received event: {event_name} for UUID {uuid}")
        
        if event_name == "CHANNEL_CREATE":
            await self.report_call_start(event)
        elif event_name == "CHANNEL_ANSWER":
            await self.report_call_answer(event)
        elif event_name == "CHANNEL_HANGUP":
            await self.report_call_end(event)

    async def report_call_start(self, event):
        payload = {
            "call_id": event.get("Unique-ID"),
            "calling_number": event.get("Caller-Caller-ID-Number"),
            "called_number": event.get("Caller-Destination-Number"),
            "start_time": datetime.now().isoformat(),
            "direction": "inbound" if event.get("Call-Direction") == "inbound" else "outbound",
            "status": "dialing"
        }
        await self.send_to_backend("/active-calls", payload)

    async def report_call_answer(self, event):
        # Update call with answer time if needed, or just status
        payload = {
            "call_id": event.get("Unique-ID"),
            "status": "answered",
            "answer_time": datetime.now().isoformat()
        }
        await self.send_to_backend("/active-calls", payload)

    async def report_call_end(self, event):
        # Post CDR and remove active call
        # Calculate duration
        duration = event.get("variable_duration") or 0
        billsec = event.get("variable_billsec") or 0
        
        # Send CDR
        cdr_payload = {
            "calling_number": event.get("Caller-Caller-ID-Number"),
            "called_number": event.get("Caller-Destination-Number"),
            "start_time": datetime.fromtimestamp(int(event.get("Caller-Channel-Created-Time")) / 1000000).isoformat(),
            "end_time": datetime.now().isoformat(),
            "duration_seconds": int(duration),
            "duration_billable": int(billsec),
            "status": event.get("Hangup-Cause"),
            "direction": "inbound" if event.get("Call-Direction") == "inbound" else "outbound",
            "release_cause": 16 # Normal clearing
        }
        await self.send_to_backend("/cdr", cdr_payload, method="POST")
        
        # Delete active call
        await self.send_to_backend(f"/active-calls/{event.get('Unique-ID')}", {}, method="DELETE")

    async def send_to_backend(self, endpoint, data, method="POST"):
        url = f"{BACKEND_URL}{endpoint}"
        async with aiohttp.ClientSession() as session:
            try:
                if method == "POST":
                    async with session.post(url, json=data) as resp:
                        logger.info(f"Sent {method} to {endpoint}: {resp.status}")
                elif method == "DELETE":
                    async with session.delete(url) as resp:
                        logger.info(f"Sent {method} to {endpoint}: {resp.status}")
            except Exception as e:
                logger.error(f"Backend request failed: {e}")

if __name__ == "__main__":
    client = ESLClient()
    loop = asyncio.get_event_loop()
    loop.run_until_complete(client.connect())
