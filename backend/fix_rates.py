import asyncio
import os
import asyncpg
from datetime import datetime

DATABASE_URL = "postgresql://apolo:apolo123@127.0.0.1:5432/apolobilling"

async def fix_dates():
    conn = await asyncpg.connect(DATABASE_URL)
    try:
        # Set all rate cards to start in the past (2020)
        await conn.execute("UPDATE rate_cards SET effective_start = '2020-01-01 00:00:00'")
        print("Updated effective_start to 2020-01-01 for all rate cards.")
        
        # Verify
        row = await conn.fetchrow("SELECT NOW(), effective_start FROM rate_cards LIMIT 1")
        print(f"DB Time: {row[0]}")
        print(f"Rate Start: {row[1]}")
    finally:
        await conn.close()

if __name__ == "__main__":
    asyncio.run(fix_dates())
