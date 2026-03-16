
import socket
import time
import uuid
import datetime

HOST = '0.0.0.0'
PORT = 8021

def format_event(event_name, headers):
    body = f"Event-Name: {event_name}\n"
    for k, v in headers.items():
        body += f"{k}: {v}\n"
    body += "\n"
    
    return f"Content-Length: {len(body)}\nContent-Type: text/event-plain\n\n{body}"

def main():
    server_socket = socket.socket(socket.AF_INET, socket.SOCK_STREAM)
    server_socket.setsockopt(socket.SOL_SOCKET, socket.SO_REUSEADDR, 1)
    
    try:
        server_socket.bind((HOST, PORT))
        server_socket.listen(1)
        print(f"📞 FreeSWITCH Simulator listening on {HOST}:{PORT}")
        
        while True:
            conn, addr = server_socket.accept()
            print(f"🔗 Connection from {addr}")
            
            try:
                # 1. Send Auth Request
                conn.sendall(b"Content-Type: auth/request\n\n")
                
                # 2. Receive Auth Command
                data = conn.recv(1024)
                print(f"📥 Received: {data.decode().strip()}")
                
                # 3. Accept Auth
                conn.sendall(b"Content-Type: command/reply\nReply-Text: +OK accepted\n\n")
                
                # 4. Handle Subscriptions (simple loop)
                # Rust engine will send "event plain ALL" or similar
                data = conn.recv(1024)
                print(f"📥 Received: {data.decode().strip()}")
                conn.sendall(b"Content-Type: command/reply\nReply-Text: +OK\n\n")
                
                print("✅ Handshake complete. Starting Call Simulation...")
                time.sleep(2)
                
                # --- START CALL SIMULATION ---
                call_uuid = str(uuid.uuid4())
                caller = "1001"
                callee = "987654321"
                
                # 1. CHANNEL_CREATE
                print(f"➡️ Sending CHANNEL_CREATE ({call_uuid})")
                create_event = format_event("CHANNEL_CREATE", {
                    "Unique-ID": call_uuid,
                    "Caller-Caller-ID-Number": caller,
                    "Caller-Destination-Number": callee,
                    "Call-Direction": "outbound",
                    "Event-Date-Timestamp": str(int(time.time() * 1000000))
                })
                conn.sendall(create_event.encode())
                
                time.sleep(2)
                
                # 2. CHANNEL_ANSWER
                print(f"➡️ Sending CHANNEL_ANSWER ({call_uuid})")
                answer_event = format_event("CHANNEL_ANSWER", {
                    "Unique-ID": call_uuid,
                    "Caller-Caller-ID-Number": caller,
                    "Caller-Destination-Number": callee,
                    "Event-Date-Timestamp": str(int(time.time() * 1000000))
                })
                conn.sendall(answer_event.encode())
                
                print("⏳ Call in progress (Billing)... waiting 5s")
                time.sleep(5)
                
                # 3. CHANNEL_HANGUP_COMPLETE
                print(f"➡️ Sending CHANNEL_HANGUP_COMPLETE ({call_uuid})")
                hangup_event = format_event("CHANNEL_HANGUP_COMPLETE", {
                    "Unique-ID": call_uuid,
                    "Caller-Caller-ID-Number": caller,
                    "Caller-Destination-Number": callee,
                    "variable_duration": "5",
                    "variable_billsec": "5",
                    "variable_hangup_cause": "NORMAL_CLEARING",
                    "Event-Date-Timestamp": str(int(time.time() * 1000000))
                })
                conn.sendall(hangup_event.encode())
                
                print("✅ Simulation cycle complete. Waiting for new connection...")
                
            except Exception as e:
                print(f"❌ Error handling connection: {e}")
            finally:
                conn.close()
                
    except Exception as e:
        print(f"❌ Server error: {e}")
    finally:
        server_socket.close()

if __name__ == "__main__":
    main()
