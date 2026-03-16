#!/usr/bin/env python3
from sqlalchemy import create_engine, text
import sys

try:
    engine = create_engine('sqlite:///./tarificador.db')
    
    with engine.connect() as conn:
        # Contar zonas
        result = conn.execute(text('SELECT COUNT(*) FROM zonas'))
        total = result.scalar()
        print(f"✅ Total de zonas en la base de datos: {total}")
        print()
        
        # Listar todas las zonas
        result = conn.execute(text('SELECT id, nombre, descripcion FROM zonas ORDER BY id'))
        zonas = result.fetchall()
        
        if zonas:
            print("📋 Zonas encontradas:")
            for zona in zonas:
                print(f"  • ID: {zona[0]}, Nombre: '{zona[1]}', Descripción: '{zona[2]}'")
            print()
        else:
            print("⚠️  No se encontraron zonas en la base de datos")
            print()
        
        # Contar prefijos por zona
        result = conn.execute(text('''
            SELECT z.id, z.nombre, COUNT(p.id) as total_prefijos
            FROM zonas z
            LEFT JOIN prefijos p ON z.id = p.zona_id
            GROUP BY z.id, z.nombre
            ORDER BY z.id
        '''))
        prefijos_por_zona = result.fetchall()
        
        if prefijos_por_zona:
            print("📊 Prefijos por zona:")
            for pz in prefijos_por_zona:
                print(f"  • Zona '{pz[1]}' (ID: {pz[0]}): {pz[2]} prefijos")
        
except Exception as e:
    print(f"❌ Error al consultar la base de datos: {e}")
    sys.exit(1)
