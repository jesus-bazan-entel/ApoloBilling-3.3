from fastapi import APIRouter, Request, Depends, HTTPException, status
from fastapi.templating import Jinja2Templates
from fastapi.responses import HTMLResponse, RedirectResponse
from fastapi.security import OAuth2PasswordBearer
from typing import Optional
import os
import json
import hmac
import hashlib
import base64
import datetime
from dotenv import load_dotenv
from sqlalchemy.orm import Session
from sqlalchemy import text
from app.db.database import get_db

load_dotenv()

from app.db.session import SessionLocal
from app.models.user import Usuario
from app.core.security import verify_password, get_password_hash
from app.schemas.user import UserCreate

# SECRET KEY for signing
SECRET_KEY = "supersecretkey"

router = APIRouter()
oauth2_scheme = OAuth2PasswordBearer(tokenUrl="token")

def get_db():
    db = SessionLocal()
    try:
        yield db
    finally:
        db.close()

# --- Custom Token Logic (No external deps) ---
def create_access_token(data: dict, expires_delta: Optional[datetime.timedelta] = None):
    to_encode = data.copy()
    if expires_delta:
        expire = datetime.datetime.utcnow() + expires_delta
    else:
        expire = datetime.datetime.utcnow() + datetime.timedelta(minutes=15)
    
    # Store expiration as ISO string to be JSON serializable
    to_encode["exp"] = expire.isoformat()
    
    # Create payload
    json_data = json.dumps(to_encode, sort_keys=True)
    msg = base64.b64encode(json_data.encode()).decode()
    
    # Sign it
    signature = hmac.new(
        SECRET_KEY.encode(), 
        msg.encode(), 
        hashlib.sha256
    ).hexdigest()
    
    return f"{msg}.{signature}"

def verify_token(token: str):
    try:
        if "." not in token:
            print(f"DEBUG: Token missing dot: {token}")
            return None
        
        msg, signature = token.split(".", 1)
        
        # Verify signature
        expected_sig = hmac.new(
            SECRET_KEY.encode(), 
            msg.encode(), 
            hashlib.sha256
        ).hexdigest()
        
        if not hmac.compare_digest(signature, expected_sig):
            print(f"DEBUG: Signature mismatch. Got {signature}, expected {expected_sig}")
            return None
            
        # Decode data
        try:
            json_data = base64.b64decode(msg).decode()
            data = json.loads(json_data)
        except Exception as e:
            print(f"DEBUG: Decode error: {e}")
            return None
        
        # Check expiration
        if "exp" in data:
            expire = datetime.datetime.fromisoformat(data["exp"])
            if datetime.datetime.utcnow() > expire:
                print(f"DEBUG: Token expired. Expires: {expire}, Now: {datetime.datetime.utcnow()}")
                return None
                
        return data
    except Exception as e:
        print(f"DEBUG: Verification exception: {e}")
        return None
# ---------------------------------------------

async def get_current_user(request: Request, db: SessionLocal = Depends(get_db)):
    print(f"DEBUG: Cookies received: {request.cookies}")
    token = request.cookies.get("access_token")
    if not token:
        print("DEBUG: No access_token cookie found")
        return None
    
    # Remove "Bearer " prefix if present
    if token.startswith("Bearer "):
        token = token.split(" ")[1]
    
    payload = verify_token(token)
    if not payload:
        print("DEBUG: verify_token returned None")
        return None
        
    username: str = payload.get("sub")
    print(f"DEBUG: Username from token: {username}")
    if username is None:
        return None
    
    user = db.query(Usuario).filter(Usuario.username == username).first()
    if not user:
        # Fallback for admin if not in DB (matching login logic)
        if username == "admin":
             return Usuario(username="admin", role="admin")
        if username == "superadmin":
             return Usuario(username="superadmin", role="superadmin")
        return None
    return user

async def get_current_active_user(user: Usuario = Depends(get_current_user)):
    if not user:
        # Redirect to login if accessing protected resource via browser? 
        # For API it should be 401. Since these are views, raising generic HTTPException might show ugly JSON.
        # But keeping it standard for now.
        raise HTTPException(status_code=status.HTTP_401_UNAUTHORIZED, detail="Not authenticated")
    return user

async def get_admin_user(user: Usuario = Depends(get_current_active_user)):
    if user.role not in ["admin", "superadmin"]:
         raise HTTPException(status_code=status.HTTP_403_FORBIDDEN, detail="Not authorized")
    return user

async def get_superadmin_user(user: Usuario = Depends(get_current_active_user)):
    if user.role != "superadmin":
         raise HTTPException(status_code=status.HTTP_403_FORBIDDEN, detail="Not authorized")
    return user

@router.get("/register", response_class=HTMLResponse)
async def register_page(request: Request):
    return templates.TemplateResponse("register.html", {"request": request, "title": "Registro"})

@router.post("/auth/register", response_class=HTMLResponse)
async def register(request: Request, db: SessionLocal = Depends(get_db)):
    form = await request.form()
    username = form.get("username")
    email = form.get("email")
    password = form.get("password")
    role = form.get("role", "operator")

    existing_user = db.query(Usuario).filter(Usuario.username == username).first()
    if existing_user:
        return templates.TemplateResponse("register.html", {
            "request": request, 
            "title": "Registro",
            "error": "El usuario ya existe"
        })

    hashed_password = get_password_hash(password)
    new_user = Usuario(
        username=username,
        email=email,
        password=hashed_password, 
        role=role
    )
    db.add(new_user)
    db.commit()
    db.refresh(new_user)

    return templates.TemplateResponse("login.html", {
            "request": request, 
            "title": "Login", 
            "message": "Usuario creado exitosamente. Por favor inicie sesión."
        })

@router.post("/auth/login", response_class=HTMLResponse)
async def login(request: Request, db: SessionLocal = Depends(get_db)):
    form = await request.form()
    username = form.get("username")
    password = form.get("password")
    
    user = db.query(Usuario).filter(Usuario.username == username).first()
    
    if not user:
        if username == "admin" and password == "admin":
             # Should create a temp admin token? Or just bypass?
             # For mock admin, let's allow it but we can't really sign a token for a user that doesn't exist 
             # unless we mock the payload.
             # Ideally admin should be in DB. 
             # Let's create a fake user obj for the context
             mock_user = Usuario(username="admin", role="admin")
             access_token = create_access_token(data={"sub": "admin"}, expires_delta=datetime.timedelta(minutes=30))
             response = RedirectResponse(url="/dashboard/cdr", status_code=status.HTTP_302_FOUND)
             response.set_cookie(key="access_token", value=f"Bearer {access_token}", httponly=True)
             return response
        
        # Superadmin check
        if username == "superadmin" and password == os.getenv("SUPERADMIN_PASSWORD"):
             mock_user = Usuario(username="superadmin", role="superadmin")
             access_token = create_access_token(data={"sub": "superadmin"}, expires_delta=datetime.timedelta(minutes=30))
             response = RedirectResponse(url="/dashboard/cdr", status_code=status.HTTP_302_FOUND)
             response.set_cookie(key="access_token", value=f"Bearer {access_token}", httponly=True)
             return response
        
        return templates.TemplateResponse("login.html", {
            "request": request, 
            "title": "Login",
            "error": "Usuario no encontrado"
        })

    if not verify_password(password, user.password):
         return templates.TemplateResponse("login.html", {
            "request": request, 
            "title": "Login",
            "error": "Contraseña incorrecta"
        })

    # Login successful
    access_token = create_access_token(
        data={"sub": user.username}, expires_delta=datetime.timedelta(minutes=30)
    )
    
    response = RedirectResponse(url="/dashboard/cdr", status_code=status.HTTP_302_FOUND)
    response.set_cookie(key="access_token", value=f"Bearer {access_token}", httponly=True)
    return response

# Utility moved up

# --- IN-MEMORY DATABASE (Global State) ---
MOCK_PLANES = [
    {"id": 1, "nombre": "Plan Ilimitado 100", "modo": "postpago", "cargo_fijo": 29.90, "minutos": 9999, "activo": True},
    {"id": 2, "nombre": "Plan Control 50", "modo": "postpago", "cargo_fijo": 19.90, "minutos": 500, "activo": True},
    {"id": 3, "nombre": "Prepago Libre", "modo": "prepago", "cargo_fijo": 0.00, "minutos": 0, "activo": True},
    {"id": 4, "nombre": "Hogar 50Mbps", "modo": "postpago", "cargo_fijo": 45.00, "minutos": 0, "activo": True},
    {"id": 5, "nombre": "Fijo Pyme", "modo": "postpago", "cargo_fijo": 30.00, "minutos": 1000, "activo": True}
]

MOCK_ABONADOS = [
    {"id": 1, "numero": "999001122", "tipo": "corporativo", "usuario": "TechCorp SAC", "area_nivel1": "Plan Ilimitado 100", "area_nivel2": "Ciclo 15", "documento_id": "20555666777", "saldo": 500.00, "activo": True},
    {"id": 2, "numero": "988776655", "tipo": "residencial", "usuario": "Juan Perez", "area_nivel1": "Hogar 50Mbps", "area_nivel2": "Ciclo 01", "documento_id": "44556677", "saldo": 25.50, "activo": True},
    {"id": 3, "numero": "012233445", "tipo": "pyme", "usuario": "Librería Central", "area_nivel1": "Fijo Pyme", "area_nivel2": "Ciclo 10", "documento_id": "10445566779", "saldo": -10.00, "activo": False},
    {"id": 4, "numero": "1001", "tipo": "anexo", "usuario": "Soporte Interno", "area_nivel1": "Plan Control 50", "area_nivel2": "Interno", "documento_id": "INT-001", "saldo": 0.00, "activo": True}
]
# -----------------------------------------


from app.models.billing import Account, RateCard

# ... (imports)

@router.get("/dashboard/accounts", response_class=HTMLResponse)
async def dashboard_accounts(request: Request, user: Usuario = Depends(get_current_active_user), db: SessionLocal = Depends(get_db)):
    accounts = db.query(Account).all()
    return templates.TemplateResponse("accounts.html", {
        "request": request,
        "title": "Gestión de Cuentas",
        "accounts": accounts,
        "user": user
    })

# Redirect old route to new one
@router.get("/dashboard/abonados")
async def redirect_abonados():
    return RedirectResponse("/dashboard/accounts")

@router.get("/dashboard/rates", response_class=HTMLResponse)
async def dashboard_rates(request: Request, user: Usuario = Depends(get_current_active_user), db: SessionLocal = Depends(get_db)):
    rates = db.query(RateCard).all()
    return templates.TemplateResponse("rates.html", {
        "request": request,
        "title": "Gestión de Tarifas",
        "rates": rates,
        "user": user
    })


# Note: /dashboard/tarifas is now served by the endpoint at line ~619 with full CRUD
# The old redirect to /dashboard/rates has been removed

@router.get("/dashboard/monitoreo", response_class=HTMLResponse)
async def dashboard_monitoreo(request: Request, user: Usuario = Depends(get_current_active_user)):
    # Standard implementation with context
    from datetime import datetime
    llamadas_recientes = [
        {"calling_number": "1001", "called_number": "987654321", "start_time": datetime.now(), "duration_seconds": 120, "cost": 0.50, "status": "disconnected"}
    ]
    return templates.TemplateResponse("monitoreo.html", {
        "request": request, 
        "title": "Monitoreo de Llamadas", 
        "user": user,
        "llamadas_hoy": 120,
        "minutos_hoy": 250,
        "alertas_saldo": [["1004", 1.50]],
        "llamadas_recientes": llamadas_recientes
    })

@router.get("/dashboard/planes", response_class=HTMLResponse)
async def dashboard_planes(request: Request, user: Usuario = Depends(get_admin_user)):
    return templates.TemplateResponse("dashboard_planes.html", {
        "request": request,
        "title": "Gestión de Planes",
        "planes": MOCK_PLANES,
        "user": user
    })

@router.get("/dashboard/abonados", response_class=HTMLResponse)
async def dashboard_abonados(
    request: Request, 
    user: Usuario = Depends(get_admin_user), 
    buscar: str = "",
    tipo_filtro: str = ""
):
    # Use global MOCK_ABONADOS
    rows = []
    for r in MOCK_ABONADOS:
        if buscar.lower() in r['numero'] or buscar.lower() in r['usuario'].lower():
             if tipo_filtro and r['tipo'] != tipo_filtro:
                 continue
             rows.append(r)

    total_records = len(rows)
    
    return templates.TemplateResponse("dashboard_abonados.html", {
        "request": request, 
        "title": "Gestión de Abonados",
        "rows": rows,
        "total_records": total_records,
        "page": 1,
        "total_pages": 1,
        "buscar": buscar,
        "tipo_filtro": tipo_filtro,
        "planes": MOCK_PLANES, 
        "areas_nivel2": ["Ciclo 01", "Ciclo 15", "Ciclo 30"],
        "pin_length": 4,
        "user": user
    })

@router.get("/dashboard/lineas")
async def redirect_lineas():
    return RedirectResponse("/dashboard/abonados")

# --- CRUD Lineas (Mock Impl) ---
from pydantic import BaseModel
class LineaCreate(BaseModel):
    numero: str
    tipo: str = "anexo"
    usuario: str
    area_nivel1: str
    area_nivel2: str | None = None
    documento_id: str | None = None
    activo: bool = True

class LineaUpdate(BaseModel):
    numero: str | None = None
    tipo: str | None = None
    usuario: str | None = None
    area_nivel1: str | None = None
    area_nivel2: str | None = None
    documento_id: str | None = None
    activo: bool | None = None

@router.post("/linea")
async def create_linea(linea: LineaCreate, user: Usuario = Depends(get_admin_user)):
    new_id = len(MOCK_ABONADOS) + 1
    new_item = {
        "id": new_id,
        "numero": linea.numero,
        "tipo": linea.tipo,
        "usuario": linea.usuario,
        "area_nivel1": linea.area_nivel1,
        "area_nivel2": linea.area_nivel2,
        "documento_id": linea.documento_id,
        "saldo": 0.00,
        "activo": linea.activo
    }
    MOCK_ABONADOS.append(new_item)
    return {"msg": "Abonado creado exitosamente", "id": new_id}

@router.get("/linea/{id}")
async def get_linea(id: int, user: Usuario = Depends(get_admin_user)):
    for item in MOCK_ABONADOS:
        if item["id"] == id:
            return item
    raise HTTPException(status_code=404, detail="Abonado no encontrado")

@router.put("/linea/{id}")
async def update_linea(id: int, linea: LineaUpdate, user: Usuario = Depends(get_admin_user)):
    for item in MOCK_ABONADOS:
        if item["id"] == id:
            if linea.numero: item["numero"] = linea.numero
            if linea.tipo: item["tipo"] = linea.tipo
            if linea.usuario: item["usuario"] = linea.usuario
            if linea.area_nivel1: item["area_nivel1"] = linea.area_nivel1
            if linea.area_nivel2: item["area_nivel2"] = linea.area_nivel2
            if linea.documento_id is not None: item["documento_id"] = linea.documento_id
            if linea.activo is not None: item["activo"] = linea.activo
            return {"msg": "Abonado actualizado", "id": id}
            
    raise HTTPException(status_code=404, detail="Abonado no encontrado")

@router.delete("/linea/{id}")
async def delete_linea(id: int, user: Usuario = Depends(get_admin_user)):
    global MOCK_ABONADOS
    for i, item in enumerate(MOCK_ABONADOS):
        if item["id"] == id:
            del MOCK_ABONADOS[i]
            return {"msg": "Abonado eliminado", "id": id}
    
    raise HTTPException(status_code=404, detail="Abonado no encontrado")




# Setup templates
# Templates are in project root (TarificadorFreeswitch/templates)
# __file__ is backend/app/web/views.py
# 1. backend/app/web
# 2. backend/app
# 3. backend
# 4. root
templates_dir = os.path.join(os.path.dirname(os.path.dirname(os.path.dirname(os.path.dirname(os.path.abspath(__file__))))), "templates")
templates = Jinja2Templates(directory=templates_dir)

# Register custom filters
def lt_filter(value, other):
    return float(value) < float(other)

def ternary_filter(condition, true_val, false_val):
    return true_val if condition else false_val

templates.env.filters["lt"] = lt_filter
templates.env.filters["ternary"] = ternary_filter

@router.get("/", response_class=HTMLResponse)
async def read_root(request: Request, user: Usuario = Depends(get_current_active_user)):
    return RedirectResponse("/dashboard/accounts")

@router.get("/login", response_class=HTMLResponse)
async def login_page(request: Request):
    return templates.TemplateResponse("login.html", {"request": request, "title": "Login"})

@router.get("/dashboard", response_class=HTMLResponse)
async def dashboard(request: Request):
    return RedirectResponse("/dashboard/metrics")



@router.get("/dashboard/saldo", response_class=HTMLResponse)
async def dashboard_saldo(request: Request, user: Usuario = Depends(get_current_active_user)):
    # Dummy data for rendering
    labels = ["1001", "1002", "1003", "1004", "1005"]
    data = [50.00, 25.50, 10.00, 0.50, 120.00]
    rows = [
        ["1001", 50.00],
        ["1002", 25.50],
        ["1003", 10.00],
        ["1004", 0.50],
        ["1005", 120.00]
    ]
    saldos_bajos = [["1004", 0.50]]
    
    return templates.TemplateResponse("dashboard_saldo.html", {
        "request": request, 
        "title": "Saldo",
        "labels": labels,
        "data": data,
        "rows": rows,
        "saldos_bajos": saldos_bajos,
        "user": user
    })



@router.get("/dashboard/finanzas", response_class=HTMLResponse)
async def dashboard_finanzas(request: Request, user: Usuario = Depends(get_current_active_user)):
    return RedirectResponse("/dashboard/facturacion")

@router.get("/dashboard/facturacion", response_class=HTMLResponse)
async def dashboard_facturacion(request: Request, user: Usuario = Depends(get_current_active_user)):
    # Mock Financial Data (Carrier style)
    labels = ["Enero", "Febrero", "Marzo", "Abril", "Mayo"]
    data = [15000.00, 18000.50, 16500.00, 22000.00, 19500.00]
    
    # Reusing dashboard_finanzas template for now as it has a generic chart
    return templates.TemplateResponse("dashboard_finanzas.html", {
        "request": request, 
        "title": "Facturación y Cobros",
        "labels": labels,
        "data": data,
        "user": user
    })

@router.get("/dashboard/zonas", response_class=HTMLResponse)
async def dashboard_zonas(request: Request, db: Session = Depends(get_db), user: Usuario = Depends(get_admin_user)):
    # Obtener zonas reales de la base de datos
    zonas_query = db.execute(text("""
        SELECT 
            z.id,
            z.zone_name,
            z.description,
            z.zone_type,
            z.region_name,
            c.country_name,
            z.zone_code
        FROM zones z
        LEFT JOIN countries c ON z.country_id = c.id
        WHERE z.enabled = true
        ORDER BY c.country_name, z.zone_name
    """)).fetchall()
    
    # Convertir a formato compatible con el template
    zonas = [
        [
            zona.id, 
            f"{zona.zone_name} ({zona.zone_type})" if zona.zone_type else zona.zone_name,
            zona.description or f"{zona.region_name} - {zona.country_name}" if zona.region_name else zona.description
        ]
        for zona in zonas_query
    ]
    
    return templates.TemplateResponse("dashboard_zonas.html", {
        "request": request,
        "title": "Gestión de Zonas",
        "zonas": zonas,
        "user": user
    })


@router.get("/dashboard/prefijos", response_class=HTMLResponse)
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
                p.operator_name,
                z.zone_name
            FROM prefixes p
            LEFT JOIN zones z ON p.zone_id = z.id
            WHERE p.zone_id = :zona_id AND p.enabled = true
            ORDER BY p.prefix
        """), {"zona_id": zona_id}).fetchall()
        
        prefijos = [
            [
                p.id,
                p.zone_id,
                p.prefix,
                p.prefix_length,
                p.operator_name or "N/A",
                p.zone_name
            ]
            for p in prefijos_query
        ]
    else:
        # Mostrar todos los prefijos
        prefijos_query = db.execute(text("""
            SELECT 
                p.id,
                p.zone_id,
                p.prefix,
                p.prefix_length,
                p.operator_name,
                z.zone_name
            FROM prefixes p
            LEFT JOIN zones z ON p.zone_id = z.id
            WHERE p.enabled = true
            ORDER BY p.prefix
            LIMIT 100
        """)).fetchall()
        
        prefijos = [
            [
                p.id,
                p.zone_id,
                p.prefix,
                p.prefix_length,
                p.operator_name or "N/A",
                p.zone_name
            ]
            for p in prefijos_query
        ]
    
    return templates.TemplateResponse("dashboard_prefijos.html", {
        "request": request,
        "title": "Gestión de Prefijos",
        "zonas": zonas,
        "prefijos": prefijos,
        "zona_actual": zona_id,
        "user": user
    })



@router.get("/dashboard/tarifas", response_class=HTMLResponse)
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
        
        tarifas = [
            [
                t.id,
                t.zone_id,
                float(t.rate_per_minute),
                t.effective_from,
                t.enabled,
                t.zone_name,
                t.rate_name,
                t.billing_increment
            ]
            for t in tarifas_query
        ]
    else:
        # Mostrar todas las tarifas activas
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
        
        tarifas = [
            [
                t.id,
                t.zone_id,
                float(t.rate_per_minute),
                t.effective_from,
                t.enabled,
                t.zone_name,
                t.rate_name,
                t.billing_increment
            ]
            for t in tarifas_query
        ]
    
    return templates.TemplateResponse("dashboard_tarifas.html", {
        "request": request,
        "title": "Gestión de Tarifas",
        "zonas": zonas,
        "tarifas": tarifas,
        "zona_actual": zona_id,
        "user": user
    })


@router.get("/dashboard/estadisticas_zona", response_class=HTMLResponse)
async def dashboard_estadisticas_zona(request: Request, user: Usuario = Depends(get_admin_user)):
    return templates.TemplateResponse("dashboard_estadisticas_zona.html", {
        "request": request,
        "title": "Estadísticas por Zona",
        "fecha_inicio": "2023-01-01",
        "fecha_fin": "2023-01-31",
        "total_llamadas": 1500,
        "total_minutos": 5000.5,
        "total_costo": 1250.75,
        "zonas_activas": 3,
        "zonas_labels": ["Nacional", "Internacional", "Móvil"],
        "llamadas_data": [1000, 200, 300],
        "costo_data": [500.0, 400.0, 350.75],
        "estadisticas": [
            {"zona_nombre": "Nacional", "total_llamadas": 1000, "duracion_total_minutos": 3000, "costo_total": 500.0, "costo_promedio": 0.5, "duracion_promedio_minutos": 3.0, "porcentaje_total": 40.0},
            {"zona_nombre": "Internacional", "total_llamadas": 200, "duracion_total_minutos": 1000, "costo_total": 400.0, "costo_promedio": 2.0, "duracion_promedio_minutos": 5.0, "porcentaje_total": 32.0}
        ],
        "user": user
    })

@router.get("/dashboard/ranking_consumo", response_class=HTMLResponse)
async def dashboard_ranking_consumo(request: Request, user: Usuario = Depends(get_current_active_user)):
    labels = ["1001", "1002", "1003"]
    data = [500.0, 300.0, 150.0]
    rows = [
        ["1001", 500.0],
        ["1002", 300.0],
        ["1003", 150.0]
    ]
    return templates.TemplateResponse("dashboard_ranking_consumo.html", {
        "request": request,
        "title": "Ranking de Consumo",
        "labels": labels,
        "data": data,
        "rows": rows
    })

@router.get("/dashboard/recargas", response_class=HTMLResponse)
async def dashboard_recargas(request: Request, user: Usuario = Depends(get_admin_user)):
    from datetime import datetime
    rows = [
        ["1001", 50.0, datetime.now(), "admin"],
        ["1002", 20.0, datetime.now(), "system"]
    ]
    return templates.TemplateResponse("dashboard_recargas.html", {
        "request": request,
        "title": "Historial de Recargas",
        "rows": rows,
        "user": user
    })

@router.get("/dashboard/auditoria", response_class=HTMLResponse)
async def dashboard_auditoria(request: Request, user: Usuario = Depends(get_admin_user)):
    # Create simple dummy template response if template exists, or redirect/error
    return templates.TemplateResponse("dashboard_auditoria.html", {
        "request": request,
        "title": "Auditoría de Cambios",
        "logs": [],
        "user": user
    })

@router.get("/dashboard/cdr", response_class=HTMLResponse)
async def dashboard_cdr_view(
    request: Request, 
    db: Session = Depends(get_db),
    user: Usuario = Depends(get_current_active_user),
    phone_number: str = "",
    start_date: str = "",
    end_date: str = "",
    status: str = "",
    direction: str = "",
    page: int = 1
):
    from datetime import datetime, timedelta
    from sqlalchemy import func, extract, case
    from app.models.cdr import CDR
    
    # Construir query base
    query = db.query(CDR)
    
    # Aplicar filtros
    if phone_number:
        query = query.filter(
            (CDR.caller_number.like(f"%{phone_number}%")) | 
            (CDR.called_number.like(f"%{phone_number}%"))
        )
    
    if start_date:
        try:
            start_dt = datetime.fromisoformat(start_date)
            query = query.filter(CDR.start_time >= start_dt)
        except:
            pass
    
    if end_date:
        try:
            end_dt = datetime.fromisoformat(end_date) + timedelta(days=1)
            query = query.filter(CDR.start_time < end_dt)
        except:
            pass
    
    if status:
        if status == "no_answer":
            query = query.filter(CDR.billsec == 0)
        elif status == "disconnected":
            query = query.filter(CDR.billsec > 0)
    
    if direction:
        query = query.filter(CDR.direction == direction)
    
    # Calcular estadísticas
    total_calls = query.count()
    completed_calls = query.filter(CDR.billsec > 0).count()
    unanswered_calls = query.filter(CDR.billsec == 0).count()
    total_cost = db.query(func.coalesce(func.sum(CDR.cost), 0.0)).filter(CDR.id.in_(query.with_entities(CDR.id))).scalar()
    total_duration = db.query(func.coalesce(func.sum(CDR.billsec), 0)).filter(CDR.id.in_(query.with_entities(CDR.id))).scalar()
    
    stats = {
        "total_calls": total_calls,
        "completed_calls": completed_calls,
        "failed_calls": 0,
        "unanswered_calls": unanswered_calls,
        "total_cost": float(total_cost) if total_cost else 0.0,
        "total_duration": int(total_duration) if total_duration else 0
    }
    
    # Datos para gráfico por hora (últimas 24 horas)
    today_start = datetime.now().replace(hour=0, minute=0, second=0, microsecond=0)
    hourly_data = db.query(
        extract('hour', CDR.start_time).label('hour'),
        func.count(CDR.id).label('count')
    ).filter(
        CDR.start_time >= today_start
    ).group_by('hour').order_by('hour').all()
    
    labels = [f"{int(h):02d}:00" for h, _ in hourly_data] if hourly_data else []
    data = [int(c) for _, c in hourly_data] if hourly_data else []
    
    # Datos para gráficos de estado y dirección
    status_data_query = db.query(
        case(
            (CDR.billsec > 0, "Contestadas"),
            else_="No Contestadas"
        ).label('status'),
        func.count(CDR.id).label('count')
    ).filter(CDR.id.in_(query.with_entities(CDR.id))).group_by('status').all()
    
    status_labels = [s for s, _ in status_data_query]
    status_data = [int(c) for _, c in status_data_query]
    
    direction_data_query = db.query(
        CDR.direction,
        func.count(CDR.id).label('count')
    ).filter(CDR.id.in_(query.with_entities(CDR.id))).group_by(CDR.direction).all()
    
    direction_labels = [d.capitalize() if d else "Desconocida" for d, _ in direction_data_query]
    direction_data = [int(c) for _, c in direction_data_query]
    
    # Paginación
    page_size = 50
    total_records = query.count()
    total_pages = (total_records + page_size - 1) // page_size
    offset = (page - 1) * page_size
    
    # Obtener CDRs para la página actual
    cdrs = query.order_by(CDR.start_time.desc()).offset(offset).limit(page_size).all()
    
    # Convertir a formato de rows para el template
    # [caller, called, start_time, ?, duration, cost, id, hangup_cause, direction, ?]
    rows = []
    for cdr in cdrs:
        rows.append([
            cdr.caller_number,           # 0: Origen
            cdr.called_number,           # 1: Destino
            cdr.start_time,              # 2: Fecha/Hora
            None,                        # 3: Usuario (no usado)
            cdr.billsec if cdr.billsec else 0,  # 4: Duración
            float(cdr.cost) if cdr.cost else 0.0,  # 5: Costo
            cdr.id,                      # 6: ID
            cdr.hangup_cause if cdr.hangup_cause else "NORMAL_CLEARING",  # 7: Estado/Causa
            cdr.direction if cdr.direction else "outbound",  # 8: Dirección
            cdr.hangup_cause if cdr.hangup_cause else "NORMAL_CLEARING",  # 9: Causa (repetido)
        ])
    
    return templates.TemplateResponse("dashboard_cdr.html", {
        "request": request, 
        "title": "Call Detail Records",
        "user": user,
        "stats": stats,
        "labels": labels,
        "data": data,
        "rows": rows,
        "status_labels": status_labels if status_labels else ["Sin datos"],
        "status_data": status_data if status_data else [0],
        "direction_labels": direction_labels if direction_labels else ["Sin datos"],
        "direction_data": direction_data if direction_data else [0],
        "page": page,
        "total_pages": total_pages if total_pages > 0 else 1
    })


@router.get("/api/reservations")
async def get_active_reservations(db: SessionLocal = Depends(get_db), user: Usuario = Depends(get_current_active_user)):
    reservations = db.query(BalanceReservation).filter(BalanceReservation.status == "active").all()
    return reservations

# Removed duplicate /dashboard/monitoreo route


@router.get("/dashboard/anexos", response_class=HTMLResponse)
async def dashboard_anexos(request: Request, user: Usuario = Depends(get_admin_user)):
    rows = [
        {"id": 1, "numero": "1001", "usuario": "Admin", "area_nivel1": "Gerencia", "area_nivel2": "TI", "area_nivel3": None, "saldo": 100.00, "pin": "1234", "activo": True},
        {"id": 2, "numero": "1002", "usuario": "User", "area_nivel1": "Ventas", "area_nivel2": None, "area_nivel3": None, "saldo": 5.00, "pin": "5678", "activo": False}
    ]
    return templates.TemplateResponse("dashboard_anexos.html", {
        "request": request,
        "title": "Gestión de Anexos",
        "rows": rows,
        "total_records": 2,
        "page": 1,
        "total_pages": 1,
        "buscar": "",
        "areas_nivel1": ["Gerencia", "Ventas", "Soporte"],
        "areas_nivel2": ["TI", "Comercial"],
        "areas_nivel3": [],
        "pin_length": 4,
        "user": user
    })

# Removed duplicate /monitoreo route (deprecated in favor of /dashboard/monitoreo)

@router.get("/config/pbx", response_class=HTMLResponse)
async def config_pbx_page(request: Request, user: Usuario = Depends(get_superadmin_user)):
    # Mock config retrieval
    config = {
        "type": "freeswitch", # default changed to freeswitch
        "host": "10.224.0.10",
        "port": 8021, # Default ESL port
        "jtapi_user": "",
        "secure_conn": False,
        "ami_user": "",
        "ami_secret": "",
        "esl_password": "ClueCon" # Default password
    }
    return templates.TemplateResponse("pbx_config.html", {
        "request": request,
        "title": "Configuración Central",
        "config": config,
        "user": user
    })

@router.post("/config/pbx", response_class=HTMLResponse)
async def config_pbx_save(request: Request, user: Usuario = Depends(get_superadmin_user)):
    form = await request.form()
    # Mock save logic
    config = {
        "type": form.get("pbx_type"),
        "host": form.get("host"),
        "port": form.get("port"),
        "jtapi_user": form.get("jtapi_user"),
        "secure_conn": form.get("secure_conn") == "on",
        "ami_user": form.get("ami_user"),
        "ami_secret": form.get("ami_secret"),
        "esl_password": form.get("esl_password")
    }
    
    return templates.TemplateResponse("pbx_config.html", {
        "request": request,
        "title": "Configuración Central",
        "config": config,
        "success": True,
        "user": user
    })

@router.get("/dashboard/pines", response_class=HTMLResponse)
async def dashboard_pines(
    request: Request, 
    user: Usuario = Depends(get_admin_user),
    buscar: str = "",
    plataforma_filtro: str = ""
):
    # Mock data for PINs/FACs
    all_rows = [
        {"id": 1, "codigo": "123456", "usuario": "Admin User", "nivel": 5, "plataforma": "cisco", "descripcion": "Acceso total", "activo": True},
        {"id": 2, "codigo": "654321", "usuario": "Basic User", "nivel": 1, "plataforma": "cisco", "descripcion": "Llamadas locales", "activo": True},
        {"id": 3, "codigo": "998877", "usuario": "Asterisk Ext 101", "nivel": None, "plataforma": "asterisk", "descripcion": "PIN salida inter", "activo": False},
        {"id": 4, "codigo": "112233", "usuario": "FS Gateway", "nivel": None, "plataforma": "freeswitch", "descripcion": "Gateway auth", "activo": True}
    ]

    # Filter logic
    rows = []
    for r in all_rows:
        if buscar.lower() in r['codigo'].lower() or buscar.lower() in r['usuario'].lower():
            if plataforma_filtro and r['plataforma'] != plataforma_filtro:
                continue
            rows.append(r)

    return templates.TemplateResponse("dashboard_pines.html", {
        "request": request, 
        "title": "Gestión de Pines",
        "rows": rows,
        "total_pages": 1,
        "page": 1,
        "total_records": len(rows),
        "buscar": buscar,
        "plataforma_filtro": plataforma_filtro,
        "user": user
    })
@router.get("/dashboard/recarga_masiva", response_class=HTMLResponse)
async def recarga_masiva_page(request: Request, user: Usuario = Depends(get_admin_user)):
    return templates.TemplateResponse("recarga_masiva.html", {"request": request, "title": "Recarga Masiva", "user": user})

@router.get("/logout", response_class=HTMLResponse)
async def logout(request: Request):
    response = templates.TemplateResponse("login.html", {"request": request, "title": "Login"})
    response.delete_cookie("access_token")
    return response

@router.get("/dashboard/metrics", response_class=HTMLResponse)
async def dashboard_metrics(request: Request, user: Usuario = Depends(get_admin_user)):
    return templates.TemplateResponse("metrics.html", {
        "request": request, 
        "title": "Métricas del Sistema", 
        "user": user
    })
async def dashboard_auditoria(request: Request, user: Usuario = Depends(get_admin_user)):
     return templates.TemplateResponse("dashboard_auditoria.html", {"request": request, "title": "Auditoría"})





# ========================================
# NEW UNIFIED RATE CARDS VIEW
# ========================================
@router.get("/dashboard/rate-cards", response_class=HTMLResponse)
async def dashboard_rate_cards(
    request: Request, 
    db: Session = Depends(get_db), 
    user: Usuario = Depends(get_admin_user)
):
    """
    Unified view for managing rate_cards (replaces zones/prefixes/tarifas)
    """
    from app.models.billing import RateCard
    from datetime import datetime
    
    # Get all active rate cards
    rates = db.query(RateCard).filter(
        (RateCard.effective_end.is_(None)) | 
        (RateCard.effective_end > datetime.utcnow())
    ).order_by(
        RateCard.destination_prefix,
        RateCard.priority.desc()
    ).all()
    
    return templates.TemplateResponse("dashboard_rate_cards.html", {
        "request": request,
        "title": "Gestión de Tarifas (Rate Cards)",
        "rates": rates,
        "user": user
    })
