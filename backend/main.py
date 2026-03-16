
from fastapi import FastAPI
from fastapi.staticfiles import StaticFiles
from fastapi.templating import Jinja2Templates
from app.api import endpoints
from app.web import views
from app.db.base_class import Base
from app.db.session import engine

# Create tables
Base.metadata.create_all(bind=engine)

app = FastAPI(title="Apolo Billing Multi-PBX")

# Custom exception handler for 401 Unauthorized (Redirect to login)
from fastapi import Request, HTTPException
from fastapi.responses import RedirectResponse

@app.exception_handler(HTTPException)
async def http_exception_handler(request: Request, exc: HTTPException):
    if exc.status_code == 401:
        # Check if it's an API call or a web page request
        if not request.url.path.startswith("/api"):
            return RedirectResponse(url="/login")
    return await request.app.default_exception_handler(request, exc)

# Mount static files
app.mount("/static", StaticFiles(directory="../static"), name="static")

from app.api.routers import accounts, rates, management, rate_cards

# Include API router
app.include_router(endpoints.router, prefix="/api")
app.include_router(accounts.router, prefix="/api/accounts", tags=["accounts"])
app.include_router(rates.router, prefix="/api/rates", tags=["rates"])
app.include_router(management.router, prefix="/api", tags=["management"])
app.include_router(rate_cards.router, prefix="/api", tags=["rate_cards"])  # ✅ NEW: Direct rate_cards API
app.include_router(views.router)

# Templates for frontend

