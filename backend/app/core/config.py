
from pydantic_settings import BaseSettings

class Settings(BaseSettings):
    PROJECT_NAME: str = "Apolo Billing"
    API_V1_STR: str = "/api"
    DATABASE_URL: str = "postgresql://tarificador_user:fr4v4t3l@localhost/tarificador"
    SECRET_KEY: str = "secreto-super-importante"
    SUPERADMIN_PASSWORD: str = "ApoloNext$Sam$"
    
    class Config:
        env_file = ".env"

settings = Settings()
