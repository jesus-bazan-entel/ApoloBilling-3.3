#!/usr/bin/env python3
"""
FreeSWITCH ESL Event Simulator para Apolo Billing Engine
Simula eventos ESL (Event Socket Layer) de FreeSWITCH para testing
"""

import socket
import time
import uuid
import argparse
from datetime import datetime
from typing import Optional

class ESLSimulator:
    """Simulador de eventos ESL de FreeSWITCH"""
    
    def __init__(self, host: str = "127.0.0.1", port: int = 8021, password: str = "ClueCon"):
        self.host = host
        self.port = port
        self.password = password
        self.sock: Optional[socket.socket] = None
        
    def connect(self) -> bool:
        """Conecta al servidor ESL"""
        try:
            self.sock = socket.socket(socket.AF_INET, socket.SOCK_STREAM)
            self.sock.connect((self.host, self.port))
            
            # Leer saludo inicial
            response = self._read_response()
            print(f"📡 Conectado a ESL: {response[:50]}...")
            
            # Autenticar
            self._send_command(f"auth {self.password}")
            auth_response = self._read_response()
            
            if "+OK accepted" in auth_response:
                print("✅ Autenticación exitosa")
                return True
            else:
                print(f"❌ Autenticación fallida: {auth_response}")
                return False
                
        except Exception as e:
            print(f"❌ Error conectando a ESL: {e}")
            return False
    
    def disconnect(self):
        """Desconecta del servidor ESL"""
        if self.sock:
            try:
                self._send_command("exit")
                self.sock.close()
                print("👋 Desconectado de ESL")
            except:
                pass
    
    def _send_command(self, command: str):
        """Envía un comando ESL"""
        if self.sock:
            self.sock.sendall(f"{command}\n\n".encode())
    
    def _read_response(self) -> str:
        """Lee una respuesta del servidor ESL"""
        response = b""
        if self.sock:
            while True:
                chunk = self.sock.recv(4096)
                if not chunk:
                    break
                response += chunk
                # ESL termina respuestas con doble newline
                if b"\n\n" in response:
                    break
        return response.decode('utf-8', errors='ignore')
    
    def send_event(self, event_name: str, headers: dict):
        """
        Envía un evento ESL personalizado
        
        Args:
            event_name: Nombre del evento (CHANNEL_CREATE, CHANNEL_ANSWER, etc.)
            headers: Diccionario con headers del evento
        """
        # Construir evento en formato ESL
        event_str = f"sendevent {event_name}\n"
        
        for key, value in headers.items():
            event_str += f"{key}: {value}\n"
        
        event_str += "\n"
        
        print(f"\n📤 Enviando evento: {event_name}")
        print(f"   UUID: {headers.get('Unique-ID', 'N/A')}")
        print(f"   Caller: {headers.get('Caller-Caller-ID-Number', 'N/A')}")
        print(f"   Callee: {headers.get('Caller-Destination-Number', 'N/A')}")
        
        self._send_command(event_str)
        
        # Leer respuesta
        response = self._read_response()
        if "+OK" in response:
            print(f"✅ Evento {event_name} enviado correctamente")
        else:
            print(f"⚠️  Respuesta: {response[:100]}")
    
    def simulate_call_flow(
        self,
        caller: str = "100001",
        callee: str = "51983434724",
        duration: int = 30,
        account_id: str = "100001"
    ):
        """
        Simula el flujo completo de una llamada:
        1. CHANNEL_CREATE (autorización)
        2. CHANNEL_ANSWER (inicio de billing)
        3. Espera (simulación de llamada)
        4. CHANNEL_HANGUP_COMPLETE (fin, CDR)
        
        Args:
            caller: Número del caller
            callee: Número destino
            duration: Duración en segundos
            account_id: ID de cuenta para billing
        """
        call_uuid = str(uuid.uuid4())
        start_epoch = int(time.time())
        
        print("\n" + "="*80)
        print(f"🎬 SIMULANDO LLAMADA COMPLETA")
        print(f"   UUID: {call_uuid}")
        print(f"   Caller: {caller}")
        print(f"   Callee: {callee}")
        print(f"   Duración esperada: {duration}s")
        print(f"   Account ID: {account_id}")
        print("="*80)
        
        # =====================================================================
        # EVENTO 1: CHANNEL_CREATE (Pre-autorización)
        # =====================================================================
        print("\n🔹 FASE 1: CHANNEL_CREATE (Autorización)")
        
        create_headers = {
            "Event-Name": "CHANNEL_CREATE",
            "Core-UUID": "d9e3a7b2-8f45-4c1e-a2d9-3f6e8b1c4a5d",
            "FreeSWITCH-Hostname": "billing-test",
            "FreeSWITCH-Switchname": "apolo-billing",
            "FreeSWITCH-IPv4": "127.0.0.1",
            "Event-Date-Local": datetime.now().strftime("%Y-%m-%d %H:%M:%S"),
            "Event-Date-GMT": datetime.utcnow().strftime("%a, %d %b %Y %H:%M:%S GMT"),
            "Event-Date-Timestamp": str(start_epoch * 1000000),
            "Event-Calling-File": "switch_core_session.c",
            "Event-Calling-Function": "switch_core_session_run",
            "Event-Calling-Line-Number": "1234",
            "Event-Sequence": "1",
            "Channel-State": "CS_INIT",
            "Channel-Call-State": "DOWN",
            "Channel-State-Number": "2",
            "Channel-Name": f"sofia/external/{callee}@gateway",
            "Unique-ID": call_uuid,
            "Call-Direction": "outbound",
            "Presence-Call-Direction": "outbound",
            "Channel-HIT-Dialplan": "true",
            "Channel-Presence-ID": f"{caller}@127.0.0.1",
            "Channel-Call-UUID": call_uuid,
            "Answer-State": "ringing",
            "Caller-Direction": "outbound",
            "Caller-Logical-Direction": "outbound",
            "Caller-Username": caller,
            "Caller-Dialplan": "XML",
            "Caller-Caller-ID-Name": caller,
            "Caller-Caller-ID-Number": caller,
            "Caller-Orig-Caller-ID-Name": caller,
            "Caller-Orig-Caller-ID-Number": caller,
            "Caller-Network-Addr": "127.0.0.1",
            "Caller-ANI": caller,
            "Caller-Destination-Number": callee,
            "Caller-Unique-ID": call_uuid,
            "Caller-Source": "mod_sofia",
            "Caller-Context": "default",
            "Caller-Channel-Name": f"sofia/external/{callee}@gateway",
            "Caller-Profile-Index": "1",
            "Caller-Profile-Created-Time": str(start_epoch * 1000000),
            "Caller-Channel-Created-Time": str(start_epoch * 1000000),
            "Caller-Channel-Answered-Time": "0",
            "Caller-Channel-Progress-Time": "0",
            "Caller-Channel-Progress-Media-Time": "0",
            "Caller-Channel-Hangup-Time": "0",
            "Caller-Channel-Transfer-Time": "0",
            "Caller-Channel-Resurrect-Time": "0",
            "Caller-Channel-Bridged-Time": "0",
            "Caller-Channel-Last-Hold": "0",
            "Caller-Channel-Hold-Accum": "0",
            "Caller-Screen-Bit": "true",
            "Caller-Privacy-Hide-Name": "false",
            "Caller-Privacy-Hide-Number": "false",
            "variable_direction": "outbound",
            "variable_uuid": call_uuid,
            "variable_session_id": "1",
            "variable_sip_from_user": caller,
            "variable_sip_from_uri": f"sip:{caller}@127.0.0.1",
            "variable_sip_to_user": callee,
            "variable_sip_to_uri": f"sip:{callee}@gateway",
            "variable_sip_call_id": f"{uuid.uuid4()}@127.0.0.1",
            "variable_account_code": account_id,
            "variable_user_context": "default",
            "variable_effective_caller_id_name": caller,
            "variable_effective_caller_id_number": caller,
            "variable_outbound_caller_id_name": caller,
            "variable_outbound_caller_id_number": caller,
            "variable_call_uuid": call_uuid,
        }
        
        self.send_event("CHANNEL_CREATE", create_headers)
        time.sleep(1)  # Esperar procesamiento de autorización
        
        # =====================================================================
        # EVENTO 2: CHANNEL_ANSWER (Inicio de facturación)
        # =====================================================================
        print("\n🔹 FASE 2: CHANNEL_ANSWER (Inicio de billing)")
        
        answer_epoch = start_epoch + 2  # Contestó 2 segundos después
        
        answer_headers = {
            "Event-Name": "CHANNEL_ANSWER",
            "Core-UUID": "d9e3a7b2-8f45-4c1e-a2d9-3f6e8b1c4a5d",
            "FreeSWITCH-Hostname": "billing-test",
            "FreeSWITCH-Switchname": "apolo-billing",
            "FreeSWITCH-IPv4": "127.0.0.1",
            "Event-Date-Local": datetime.fromtimestamp(answer_epoch).strftime("%Y-%m-%d %H:%M:%S"),
            "Event-Date-GMT": datetime.utcfromtimestamp(answer_epoch).strftime("%a, %d %b %Y %H:%M:%S GMT"),
            "Event-Date-Timestamp": str(answer_epoch * 1000000),
            "Event-Calling-File": "switch_core_session.c",
            "Event-Calling-Function": "switch_core_session_run",
            "Event-Calling-Line-Number": "2345",
            "Event-Sequence": "2",
            "Channel-State": "CS_EXECUTE",
            "Channel-Call-State": "ACTIVE",
            "Channel-State-Number": "4",
            "Channel-Name": f"sofia/external/{callee}@gateway",
            "Unique-ID": call_uuid,
            "Call-Direction": "outbound",
            "Presence-Call-Direction": "outbound",
            "Channel-HIT-Dialplan": "true",
            "Channel-Presence-ID": f"{caller}@127.0.0.1",
            "Channel-Call-UUID": call_uuid,
            "Answer-State": "answered",
            "Caller-Direction": "outbound",
            "Caller-Logical-Direction": "outbound",
            "Caller-Username": caller,
            "Caller-Dialplan": "XML",
            "Caller-Caller-ID-Name": caller,
            "Caller-Caller-ID-Number": caller,
            "Caller-Orig-Caller-ID-Name": caller,
            "Caller-Orig-Caller-ID-Number": caller,
            "Caller-Network-Addr": "127.0.0.1",
            "Caller-ANI": caller,
            "Caller-Destination-Number": callee,
            "Caller-Unique-ID": call_uuid,
            "Caller-Source": "mod_sofia",
            "Caller-Context": "default",
            "Caller-Channel-Name": f"sofia/external/{callee}@gateway",
            "Caller-Profile-Index": "1",
            "Caller-Profile-Created-Time": str(start_epoch * 1000000),
            "Caller-Channel-Created-Time": str(start_epoch * 1000000),
            "Caller-Channel-Answered-Time": str(answer_epoch * 1000000),
            "Caller-Channel-Progress-Time": str((start_epoch + 1) * 1000000),
            "Caller-Channel-Progress-Media-Time": str((start_epoch + 1) * 1000000),
            "Caller-Channel-Hangup-Time": "0",
            "Caller-Channel-Transfer-Time": "0",
            "Caller-Channel-Resurrect-Time": "0",
            "Caller-Channel-Bridged-Time": "0",
            "Caller-Channel-Last-Hold": "0",
            "Caller-Channel-Hold-Accum": "0",
            "Caller-Screen-Bit": "true",
            "Caller-Privacy-Hide-Name": "false",
            "Caller-Privacy-Hide-Number": "false",
            "variable_direction": "outbound",
            "variable_uuid": call_uuid,
            "variable_session_id": "1",
            "variable_sip_from_user": caller,
            "variable_sip_from_uri": f"sip:{caller}@127.0.0.1",
            "variable_sip_to_user": callee,
            "variable_sip_to_uri": f"sip:{callee}@gateway",
            "variable_sip_call_id": f"{uuid.uuid4()}@127.0.0.1",
            "variable_account_code": account_id,
            "variable_user_context": "default",
            "variable_effective_caller_id_name": caller,
            "variable_effective_caller_id_number": caller,
            "variable_outbound_caller_id_name": caller,
            "variable_outbound_caller_id_number": caller,
            "variable_call_uuid": call_uuid,
            "variable_answersec": "2",
            "variable_answer_epoch": str(answer_epoch),
            "variable_answer_uepoch": str(answer_epoch * 1000000),
        }
        
        self.send_event("CHANNEL_ANSWER", answer_headers)
        
        # =====================================================================
        # SIMULACIÓN DE LLAMADA EN CURSO
        # =====================================================================
        print(f"\n⏳ Simulando llamada en curso ({duration} segundos)...")
        print("   (El motor de billing Rust está facturando en tiempo real)")
        
        for i in range(duration):
            if i % 10 == 0 and i > 0:
                print(f"   ⏱️  {i}s transcurridos...")
            time.sleep(1)
        
        # =====================================================================
        # EVENTO 3: CHANNEL_HANGUP_COMPLETE (Fin de llamada, CDR)
        # =====================================================================
        print(f"\n🔹 FASE 3: CHANNEL_HANGUP_COMPLETE (Generación de CDR)")
        
        hangup_epoch = answer_epoch + duration
        billsec = duration  # Tiempo facturable
        total_duration = hangup_epoch - start_epoch
        
        hangup_headers = {
            "Event-Name": "CHANNEL_HANGUP_COMPLETE",
            "Core-UUID": "d9e3a7b2-8f45-4c1e-a2d9-3f6e8b1c4a5d",
            "FreeSWITCH-Hostname": "billing-test",
            "FreeSWITCH-Switchname": "apolo-billing",
            "FreeSWITCH-IPv4": "127.0.0.1",
            "Event-Date-Local": datetime.fromtimestamp(hangup_epoch).strftime("%Y-%m-%d %H:%M:%S"),
            "Event-Date-GMT": datetime.utcfromtimestamp(hangup_epoch).strftime("%a, %d %b %Y %H:%M:%S GMT"),
            "Event-Date-Timestamp": str(hangup_epoch * 1000000),
            "Event-Calling-File": "switch_core_session.c",
            "Event-Calling-Function": "switch_core_session_destroy",
            "Event-Calling-Line-Number": "3456",
            "Event-Sequence": "3",
            "Channel-State": "CS_DESTROY",
            "Channel-Call-State": "HANGUP",
            "Channel-State-Number": "10",
            "Channel-Name": f"sofia/external/{callee}@gateway",
            "Unique-ID": call_uuid,
            "Call-Direction": "outbound",
            "Presence-Call-Direction": "outbound",
            "Channel-HIT-Dialplan": "true",
            "Channel-Presence-ID": f"{caller}@127.0.0.1",
            "Channel-Call-UUID": call_uuid,
            "Answer-State": "hangup",
            "Hangup-Cause": "NORMAL_CLEARING",
            "Caller-Direction": "outbound",
            "Caller-Logical-Direction": "outbound",
            "Caller-Username": caller,
            "Caller-Dialplan": "XML",
            "Caller-Caller-ID-Name": caller,
            "Caller-Caller-ID-Number": caller,
            "Caller-Orig-Caller-ID-Name": caller,
            "Caller-Orig-Caller-ID-Number": caller,
            "Caller-Network-Addr": "127.0.0.1",
            "Caller-ANI": caller,
            "Caller-Destination-Number": callee,
            "Caller-Unique-ID": call_uuid,
            "Caller-Source": "mod_sofia",
            "Caller-Context": "default",
            "Caller-Channel-Name": f"sofia/external/{callee}@gateway",
            "Caller-Profile-Index": "1",
            "Caller-Profile-Created-Time": str(start_epoch * 1000000),
            "Caller-Channel-Created-Time": str(start_epoch * 1000000),
            "Caller-Channel-Answered-Time": str(answer_epoch * 1000000),
            "Caller-Channel-Progress-Time": str((start_epoch + 1) * 1000000),
            "Caller-Channel-Progress-Media-Time": str((start_epoch + 1) * 1000000),
            "Caller-Channel-Hangup-Time": str(hangup_epoch * 1000000),
            "Caller-Channel-Transfer-Time": "0",
            "Caller-Channel-Resurrect-Time": "0",
            "Caller-Channel-Bridged-Time": "0",
            "Caller-Channel-Last-Hold": "0",
            "Caller-Channel-Hold-Accum": "0",
            "Caller-Screen-Bit": "true",
            "Caller-Privacy-Hide-Name": "false",
            "Caller-Privacy-Hide-Number": "false",
            "variable_direction": "outbound",
            "variable_uuid": call_uuid,
            "variable_session_id": "1",
            "variable_sip_from_user": caller,
            "variable_sip_from_uri": f"sip:{caller}@127.0.0.1",
            "variable_sip_to_user": callee,
            "variable_sip_to_uri": f"sip:{callee}@gateway",
            "variable_sip_call_id": f"{uuid.uuid4()}@127.0.0.1",
            "variable_account_code": account_id,
            "variable_user_context": "default",
            "variable_effective_caller_id_name": caller,
            "variable_effective_caller_id_number": caller,
            "variable_outbound_caller_id_name": caller,
            "variable_outbound_caller_id_number": caller,
            "variable_call_uuid": call_uuid,
            "variable_answersec": "2",
            "variable_answer_epoch": str(answer_epoch),
            "variable_answer_uepoch": str(answer_epoch * 1000000),
            "variable_start_epoch": str(start_epoch),
            "variable_start_uepoch": str(start_epoch * 1000000),
            "variable_end_epoch": str(hangup_epoch),
            "variable_end_uepoch": str(hangup_epoch * 1000000),
            "variable_duration": str(total_duration),
            "variable_billsec": str(billsec),
            "variable_progresssec": "1",
            "variable_answersec": "2",
            "variable_waitsec": "1",
            "variable_progress_mediasec": "1",
            "variable_flow_billsec": str(billsec),
            "variable_mduration": str(total_duration * 1000),
            "variable_billmsec": str(billsec * 1000),
            "variable_progressmsec": "1000",
            "variable_answermsec": "2000",
            "variable_waitmsec": "1000",
            "variable_progress_mediamsec": "1000",
            "variable_flow_billmsec": str(billsec * 1000),
            "variable_uduration": str(total_duration * 1000000),
            "variable_billusec": str(billsec * 1000000),
            "variable_progressusec": "1000000",
            "variable_answerusec": "2000000",
            "variable_waitusec": "1000000",
            "variable_progress_mediausec": "1000000",
            "variable_flow_billusec": str(billsec * 1000000),
            "variable_hangup_cause": "NORMAL_CLEARING",
            "variable_hangup_cause_q850": "16",
            "variable_sip_hangup_disposition": "send_bye",
        }
        
        self.send_event("CHANNEL_HANGUP_COMPLETE", hangup_headers)
        
        print("\n✅ Simulación de llamada completada")
        print(f"   Total duration: {total_duration}s")
        print(f"   Billable seconds: {billsec}s")
        print(f"   Hangup cause: NORMAL_CLEARING")
        

def main():
    parser = argparse.ArgumentParser(
        description="Simulador de eventos ESL de FreeSWITCH para Apolo Billing Engine"
    )
    parser.add_argument("--host", default="127.0.0.1", help="Host del ESL (default: 127.0.0.1)")
    parser.add_argument("--port", type=int, default=8021, help="Puerto ESL (default: 8021)")
    parser.add_argument("--password", default="ClueCon", help="Password ESL (default: ClueCon)")
    parser.add_argument("--caller", default="100001", help="Número del caller (default: 100001)")
    parser.add_argument("--callee", default="51987654321", help="Número destino (default: 51987654321)")
    parser.add_argument("--duration", type=int, default=60, help="Duración de llamada en segundos (default: 60)")
    parser.add_argument("--account", default="100001", help="Account ID para billing (default: 100001)")
    parser.add_argument("--calls", type=int, default=1, help="Número de llamadas a simular (default: 1)")
    parser.add_argument("--delay", type=int, default=5, help="Delay entre llamadas en segundos (default: 5)")
    
    args = parser.parse_args()
    
    print("╔══════════════════════════════════════════════════════════════════════╗")
    print("║         FreeSWITCH ESL Event Simulator - Apolo Billing Engine       ║")
    print("╚══════════════════════════════════════════════════════════════════════╝")
    print()
    print(f"🔧 Configuración:")
    print(f"   ESL Host: {args.host}:{args.port}")
    print(f"   Caller: {args.caller}")
    print(f"   Callee: {args.callee}")
    print(f"   Duration: {args.duration}s")
    print(f"   Account: {args.account}")
    print(f"   Calls: {args.calls}")
    print()
    
    simulator = ESLSimulator(args.host, args.port, args.password)
    
    if not simulator.connect():
        print("❌ No se pudo conectar al servidor ESL")
        print("\n💡 Asegúrate de que el motor Rust esté corriendo:")
        print("   cd rust-billing-engine && cargo run")
        return
    
    try:
        for i in range(args.calls):
            if args.calls > 1:
                print(f"\n{'='*80}")
                print(f"📞 LLAMADA {i+1} de {args.calls}")
                print(f"{'='*80}")
            
            simulator.simulate_call_flow(
                caller=args.caller,
                callee=args.callee,
                duration=args.duration,
                account_id=args.account
            )
            
            if i < args.calls - 1:
                print(f"\n⏳ Esperando {args.delay}s antes de la siguiente llamada...")
                time.sleep(args.delay)
        
        print("\n" + "="*80)
        print("🎉 SIMULACIÓN COMPLETADA")
        print("="*80)
        print("\n💡 Verifica los resultados:")
        print("   • Logs del motor Rust")
        print("   • Base de datos PostgreSQL (tabla cdrs)")
        print("   • Redis (reservaciones de balance)")
        print("   • Dashboard: http://localhost:8000/dashboard/cdr")
        
    except KeyboardInterrupt:
        print("\n⚠️  Simulación interrumpida por el usuario")
    finally:
        simulator.disconnect()


if __name__ == "__main__":
    main()
