#!/usr/bin/env python3
"""
Script para actualizar las funciones de zonas, prefijos y tarifas en main.py
para que usen la base de datos real en lugar de datos hardcodeados
"""

import re

def update_main_py():
    print("📝 Leyendo main.py...")
    
    with open('main.py', 'r', encoding='utf-8') as f:
        content = f.read()
    
    # Hacer backup
    print("💾 Creando backup...")
    with open('main.py.backup', 'w', encoding='utf-8') as f:
        f.write(content)
    
    # 1. Actualizar dashboard_zonas
    print("🔄 Actualizando dashboard_zonas...")
    zonas_pattern = r'@router\.get\("/dashboard/zonas".*?\n(?=@router\.get|@app\.get|@app\.post|def |class )'
    
    zonas_replacement = '''@app.get("/dashboard/zonas", response_class=HTMLResponse)
async def dashboard_zonas(request: Request, db: Session = Depends(get_db), user: Usuario = Depends(get_admin_user)):
    # Obtener zonas reales de la base de datos
    zonas_query = db.execute(text("""
        SELECT 
            z.id,
            z.zone_name,
            COALESCE(z.description, CONCAT(COALESCE(z.region_name, ''), ' - ', COALESCE(c.country_name, ''))) as description
        FROM zones z
        LEFT JOIN countries c ON z.country_id = c.id
        WHERE z.enabled = true
        ORDER BY c.country_name, z.zone_name
    """)).fetchall()
    
    # Convertir a formato compatible con el template
    zonas = [[zona.id, zona.zone_name, zona.description] for zona in zonas_query]
    
    return templates.TemplateResponse("dashboard_zonas.html", {
        "request": request,
        "title": "Gestión de Zonas",
        "zonas": zonas,
        "user": user
    })

'''
    
    content = re.sub(zonas_pattern, zonas_replacement, content, flags=re.DOTALL)
    
    # 2. Actualizar dashboard_prefijos
    print("🔄 Actualizando dashboard_prefijos...")
    prefijos_pattern = r'@router\.get\("/dashboard/prefijos".*?\n(?=@router\.get|@app\.get|@app\.post|def |class )'
    
    prefijos_replacement = '''@app.get("/dashboard/prefijos", response_class=HTMLResponse)
async def dashboard_prefijos(request: Request, db: Session = Depends(get_db), user: Usuario = Depends(get_admin_user), zona_id: int = None):
    # Obtener zonas para el dropdown
    zonas_query = db.execute(text("""
        SELECT id, zone_name 
        FROM zones 
        WHERE enabled = true
        ORDER BY zone_name
    """)).fetchall()
    zonas = [[z.id, z.zone_name] for z in zonas_query]
    
    # Obtener prefijos
    if zona_id:
        prefijos_query = db.execute(text("""
            SELECT 
                p.id,
                p.zone_id,
                p.prefix,
                p.prefix_length,
                p.prefix_length,
                z.zone_name,
                COALESCE(p.operator_name, 'N/A')
            FROM prefixes p
            LEFT JOIN zones z ON p.zone_id = z.id
            WHERE p.zone_id = :zona_id AND p.enabled = true
            ORDER BY p.prefix
        """), {"zona_id": zona_id}).fetchall()
        
        prefijos = [[p[0], p[1], p[2], p[3], p[4], p[5], p[6]] for p in prefijos_query]
        zona_actual = [zona_id, next((z[1] for z in zonas if z[0] == zona_id), None)]
    else:
        prefijos_query = db.execute(text("""
            SELECT 
                p.id,
                p.zone_id,
                p.prefix,
                p.prefix_length,
                p.prefix_length,
                z.zone_name,
                COALESCE(p.operator_name, 'N/A')
            FROM prefixes p
            LEFT JOIN zones z ON p.zone_id = z.id
            WHERE p.enabled = true
            ORDER BY p.prefix
            LIMIT 100
        """)).fetchall()
        
        prefijos = [[p[0], p[1], p[2], p[3], p[4], p[5], p[6]] for p in prefijos_query]
        zona_actual = None
    
    return templates.TemplateResponse("dashboard_prefijos.html", {
        "request": request,
        "title": "Gestión de Prefijos",
        "zonas": zonas,
        "prefijos": prefijos,
        "zona_actual": zona_actual,
        "user": user
    })

'''
    
    content = re.sub(prefijos_pattern, prefijos_replacement, content, flags=re.DOTALL)
    
    # 3. Actualizar dashboard_tarifas
    print("🔄 Actualizando dashboard_tarifas...")
    tarifas_pattern = r'@router\.get\("/dashboard/tarifas".*?\n(?=@router\.get|@app\.get|@app\.post|def |class )'
    
    tarifas_replacement = '''@app.get("/dashboard/tarifas", response_class=HTMLResponse)
async def dashboard_tarifas(request: Request, db: Session = Depends(get_db), user: Usuario = Depends(get_admin_user), zona_id: int = None):
    # Obtener zonas para el dropdown
    zonas_query = db.execute(text("""
        SELECT id, zone_name 
        FROM zones 
        WHERE enabled = true
        ORDER BY zone_name
    """)).fetchall()
    zonas = [[z.id, z.zone_name] for z in zonas_query]
    
    # Obtener tarifas
    if zona_id:
        tarifas_query = db.execute(text("""
            SELECT 
                rz.id,
                rz.zone_id,
                rz.rate_per_minute,
                rz.effective_from,
                rz.enabled,
                z.zone_name,
                rz.rate_name,
                rz.billing_increment
            FROM rate_zones rz
            LEFT JOIN zones z ON rz.zone_id = z.id
            WHERE rz.zone_id = :zona_id
            ORDER BY rz.effective_from DESC
        """), {"zona_id": zona_id}).fetchall()
        
        tarifas = [[t[0], t[1], float(t[2]), t[3], t[4], t[5], t[6], t[7]] for t in tarifas_query]
        zona_actual = [zona_id, next((z[1] for z in zonas if z[0] == zona_id), None)]
    else:
        tarifas_query = db.execute(text("""
            SELECT 
                rz.id,
                rz.zone_id,
                rz.rate_per_minute,
                rz.effective_from,
                rz.enabled,
                z.zone_name,
                rz.rate_name,
                rz.billing_increment
            FROM rate_zones rz
            LEFT JOIN zones z ON rz.zone_id = z.id
            WHERE rz.enabled = true
            ORDER BY z.zone_name, rz.effective_from DESC
            LIMIT 100
        """)).fetchall()
        
        tarifas = [[t[0], t[1], float(t[2]), t[3], t[4], t[5], t[6], t[7]] for t in tarifas_query]
        zona_actual = None
    
    return templates.TemplateResponse("dashboard_tarifas.html", {
        "request": request,
        "title": "Gestión de Tarifas",
        "zonas": zonas,
        "tarifas": tarifas,
        "zona_actual": zona_actual,
        "user": user
    })

'''
    
    content = re.sub(tarifas_pattern, tarifas_replacement, content, flags=re.DOTALL)
    
    # Guardar cambios
    print("💾 Guardando cambios...")
    with open('main.py', 'w', encoding='utf-8') as f:
        f.write(content)
    
    print("✅ main.py actualizado correctamente")
    print("📝 Backup guardado en main.py.backup")

if __name__ == "__main__":
    update_main_py()
