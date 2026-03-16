
import asyncio
import random

async def handle_client(reader, writer):
    print("New client connected")
    
    # Simulate auth
    writer.write(b"Content-Type: auth/request\n\n")
    await writer.drain()
    
    auth_line = await reader.readline()
    print(f"Received: {auth_line}")
    # Consume empty line
    await reader.readline()
    
    writer.write(b"Content-Type: command/reply\nReply-Text: +OK accepted\n\n")
    await writer.drain()
    
    # Command loop
    while True:
        line = await reader.readline()
        if not line: break
        
        # Consume rest of command
        if line == b"\n":
            continue
            
        print(f"Command: {line}")
        
        if b"events" in line:
            writer.write(b"Content-Type: command/reply\nReply-Text: +OK event listener enabled plain\n\n")
            await writer.drain()
            
            # Start generating events
            asyncio.create_task(generate_events(writer))

async def generate_events(writer):
    i = 0
    while True:
        await asyncio.sleep(5)
        i += 1
        uuid = f"test-call-{i}"
        
        # CHANNEL_CREATE
        event = f"""Content-Length: 550
Content-Type: text/event-plain

Event-Name: CHANNEL_CREATE
Unique-ID: {uuid}
Caller-Caller-ID-Number: 100{i}
Caller-Destination-Number: 200{i}
Call-Direction: outbound
Caller-Channel-Created-Time: {int(asyncio.get_event_loop().time() * 1000000)}
\n"""
        try:
            writer.write(event.encode())
            await writer.drain()
            print(f"Sent CHANNEL_CREATE for {uuid}")
        except:
            break
            
        await asyncio.sleep(3)
        
        # CHANNEL_HANGUP
        event = f"""Content-Length: 550
Content-Type: text/event-plain

Event-Name: CHANNEL_HANGUP
Unique-ID: {uuid}
Caller-Caller-ID-Number: 100{i}
Caller-Destination-Number: 200{i}
Call-Direction: outbound
Caller-Channel-Created-Time: {int(asyncio.get_event_loop().time() * 1000000)}
variable_duration: 3
variable_billsec: 3
Hangup-Cause: NORMAL_CLEARING
\n"""
        try:
            writer.write(event.encode())
            await writer.drain()
            print(f"Sent CHANNEL_HANGUP for {uuid}")
        except:
            break

async def main():
    server = await asyncio.start_server(handle_client, '127.0.0.1', 8021)
    print("Mock Freeswitch ESL Server running on 8021...")
    async with server:
        await server.serve_forever()

if __name__ == "__main__":
    asyncio.run(main())
