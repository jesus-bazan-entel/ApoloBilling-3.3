from pydantic import BaseModel
from typing import Optional

class UserBase(BaseModel):
    username: str
    email: Optional[str] = None
    role: Optional[str] = "operator"

class UserCreate(UserBase):
    password: str
    nombre: Optional[str] = None
    apellido: Optional[str] = None

class UserLogin(BaseModel):
    username: str
    password: str

class UserResponse(UserBase):
    id: int
    activo: bool
    
    class Config:
        from_attributes = True
