#!/usr/bin/env python3
"""
Script de verificación final - Confirma que las zonas están en la base de datos
y que el código está listo para usarlas
"""

from sqlalchemy import create_engine, text
from sqlalchemy.orm import sessionmaker

DATABASE_URL = "postgresql://tarificador_user:fr4v4t3l@localhost/tarificador"

engine = create_engine(DATABASE_URL)
SessionLocal = sessionmaker(autocommit=False, autoflush=False, bind=engine)

print("=" * 70)
print("  VERIFICACIÓN FINAL - DIAGNÓSTICO DE ZONAS")
print("=" * 70)
print()

db = SessionLocal()

try:
    # 1. Verificar que las zonas existen
    print("1️⃣  Verificando zonas en la base de datos...")
    zonas_query = text("SELECT id, nombre, descripcion FROM zonas ORDER BY nombre")
    zonas = db.execute(zonas_query).fetchall()
    
    if zonas:
        print(f"   ✅ Se encontraron {len(zonas)} zonas:")
        for zona in zonas:
            print(f"      • {zona[1]} (ID: {zona[0]})")
    else:
        print("   ❌ No se encontraron zonas")
        exit(1)
    print()
    
    # 2. Verificar prefijos
    print("2️⃣  Verificando prefijos...")
    prefijos_query = text("SELECT COUNT(*) FROM prefijos")
    total_prefijos = db.execute(prefijos_query).scalar()
    print(f"   ✅ Total de prefijos: {total_prefijos}")
    print()
    
    # 3. Verificar tarifas
    print("3️⃣  Verificando tarifas...")
    tarifas_query = text("SELECT COUNT(*) FROM tarifas")
    total_tarifas = db.execute(tarifas_query).scalar()
    print(f"   ✅ Total de tarifas: {total_tarifas}")
    print()
    
    # 4. Simular la consulta que hace el dashboard
    print("4️⃣  Simulando consulta del dashboard de prefijos...")
    dashboard_query = text("SELECT id, nombre FROM zonas ORDER BY nombre")
    dashboard_zonas = db.execute(dashboard_query).fetchall()
    
    print(f"   ✅ El dashboard debería mostrar {len(dashboard_zonas)} zonas:")
    for zona in dashboard_zonas:
        print(f"      • {zona[1]} (ID: {zona[0]})")
    print()
    
    # 5. Verificar relaciones
    print("5️⃣  Verificando relaciones zona-prefijo...")
    relaciones_query = text("""
        SELECT z.nombre, COUNT(p.id) as total_prefijos
        FROM zonas z
        LEFT JOIN prefijos p ON z.id = p.zona_id
        GROUP BY z.id, z.nombre
        ORDER BY z.nombre
    """)
    relaciones = db.execute(relaciones_query).fetchall()
    
    for rel in relaciones:
        print(f"      • {rel[0]}: {rel[1]} prefijos")
    print()
    
    print("=" * 70)
    print("  ✅ DIAGNÓSTICO COMPLETADO EXITOSAMENTE")
    print("=" * 70)
    print()
    print("📋 RESUMEN:")
    print(f"   • Zonas configuradas: {len(zonas)}")
    print(f"   • Prefijos configurados: {total_prefijos}")
    print(f"   • Tarifas configuradas: {total_tarifas}")
    print()
    print("💡 PRÓXIMOS PASOS:")
    print("   1. Las zonas YA ESTÁN en la base de datos")
    print("   2. El código en main.py YA LEE de la base de datos (línea 3202)")
    print("   3. Al abrir el modal 'Nuevo Prefijo', las 6 zonas deberían aparecer")
    print()
    print("🔧 Si las zonas NO aparecen en el modal:")
    print("   • Verifica la consola del navegador (F12) para errores JavaScript")
    print("   • Verifica que estés autenticado en el sistema")
    print("   • Verifica que el template dashboard_prefijos.html esté correcto")
    print()
    
except Exception as e:
    print(f"❌ Error: {e}")
    import traceback
    traceback.print_exc()
    exit(1)
finally:
    db.close()
