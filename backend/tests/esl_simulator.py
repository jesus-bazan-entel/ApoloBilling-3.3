import asyncio
import logging
import sys

# Configure logging
logging.basicConfig(
    level=logging.INFO,
    format='%(asctime)s - %(name)s - %(levelname)s - %(message)s'
)
logger = logging.getLogger("ESLSimulator")

HOST = '127.0.0.1'
PORT = 8021

class ESLProtocol(asyncio.Protocol):
    def __init__(self):
        self.transport = None
        self.peername = None

    def connection_made(self, transport):
        self.transport = transport
        self.peername = transport.get_extra_info('peername')
        logger.info(f"Connection from {self.peername}")
        
        # Send initial authentication request
        self.send_command("Content-Type", "auth/request")

    def data_received(self, data):
        message = data.decode()
        logger.debug(f"Received: {message}")
        
        lines = message.strip().split('\n')
        command_line = lines[0] if lines else ""

        if "auth" in command_line:
            logger.info("Received auth request, accepting...")
            self.send_reply("+OK accepted")
        elif "events" in command_line:
            logger.info(f"Received events subscription: {command_line}")
            self.send_reply("+OK event listener enabled plain")
        elif "filter" in command_line:
            logger.info(f"Received filter command: {command_line}")
            self.send_reply("+OK filter added")
        elif "api" in command_line:
            logger.info(f"Received api command: {command_line}")
            # Mock responses for specific api commands if needed
            if "uuid_kill" in command_line:
                 self.send_reply("+OK killed")
            else:
                 self.send_reply("+OK")
        elif "exit" in command_line:
            logger.info("Closing connection")
            self.transport.close()
        else:
            logger.info(f"Unknown command: {command_line}")
            # Default reply to keep client happy
            self.send_reply("+OK")

    def send_command(self, header, value):
        msg = f"{header}: {value}\n\n"
        self.transport.write(msg.encode())

    def send_reply(self, message):
        response = f"Content-Type: command/reply\nReply-Text: {message}\n\n"
        self.transport.write(response.encode())
    
    def send_event(self, event_name, headers):
        """Send an arbitrary event to the client"""
        msg = f"Content-Type: text/event-plain\n"
        for k, v in headers.items():
            msg += f"{k}: {v}\n"
        # Ensure event name is in header
        if "Event-Name" not in headers:
            msg += f"Event-Name: {event_name}\n"
        
        # Add body length if needed, for now simple events
        msg += f"\n" # End of headers
        
        logger.info(f"Sending event {event_name}")
        self.transport.write(msg.encode())

    def connection_lost(self, exc):
        logger.info(f"Connection lost from {self.peername}")
        # Remove from global clients list if we were tracking multiples
        if self in clients:
            clients.remove(self)

clients = []

async def console_input_loop():
    logger.info("Simulator ready. Type commands to generate events:")
    logger.info("  park <uuid> <caller> <callee>  - Simulate CHANNEL_CREATE (New Call)")
    logger.info("  answer <uuid>                  - Simulate CHANNEL_ANSWER")
    logger.info("  hangup <uuid>                  - Simulate CHANNEL_HANGUP")
    logger.info("  quit                           - Exit simulator")

    loop = asyncio.get_event_loop()
    reader = asyncio.StreamReader()
    protocol = asyncio.StreamReaderProtocol(reader)
    await loop.connect_read_pipe(lambda: protocol, sys.stdin)

    while True:
        print("> ", end='', flush=True)
        line = await reader.readline()
        if not line:
            break
        
        cmd = line.decode().strip().split()
        if not cmd:
            continue
            
        action = cmd[0].lower()
        
        if action == "quit":
            break
        
        if not clients:
            logger.warning("No clients connected. Connect your Billing Engine first.")
            continue
            
        # Broadcast to all connected clients (usually just one engine)
        for client in clients:
            if action == 'park':
                if len(cmd) < 4:
                    print("Usage: park <uuid> <caller> <callee>")
                    continue
                uuid, caller, callee = cmd[1], cmd[2], cmd[3]
                client.send_event("CHANNEL_CREATE", {
                    "Unique-ID": uuid,
                    "Caller-Caller-ID-Number": caller,
                    "Caller-Destination-Number": callee,
                    "Channel-State": "CS_EXCHANGE_MEDIA",
                    "Answer-State": "ringing"
                })
            
            elif action == 'answer':
                if len(cmd) < 2:
                    print("Usage: answer <uuid>")
                    continue
                uuid = cmd[1]
                client.send_event("CHANNEL_ANSWER", {
                    "Unique-ID": uuid,
                    "Channel-State": "CS_EXCHANGE_MEDIA",
                    "Answer-State": "answered"
                })
                
            elif action == 'hangup':
                if len(cmd) < 2:
                    print("Usage: hangup <uuid>")
                    continue
                uuid = cmd[1]
                client.send_event("CHANNEL_HANGUP", {
                    "Unique-ID": uuid,
                    "Hangup-Cause": "NORMAL_CLEARING"
                })
            
            else:
                print(f"Unknown command: {action}")

async def main():
    loop = asyncio.get_running_loop()
    
    def create_protocol():
        proto = ESLProtocol()
        clients.append(proto)
        return proto

    server = await loop.create_server(create_protocol, HOST, PORT)
    logger.info(f"ESL Simulator listening on {HOST}:{PORT}")
    
    await asyncio.gather(
        server.serve_forever(),
        console_input_loop()
    )

if __name__ == "__main__":
    try:
        asyncio.run(main())
    except KeyboardInterrupt:
        pass
