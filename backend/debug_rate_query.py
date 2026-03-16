import asyncio
import asyncpg
import datetime

DATABASE_URL = "postgresql://apolo:apolo123@127.0.0.1:5432/apolobilling"

async def debug_rate():
    conn = await asyncpg.connect(DATABASE_URL)
    try:
        print("--- Debugging Rate Lookup ---")
        
        # 1. Check DB Time
        now = await conn.fetchval("SELECT NOW()")
        print(f"DB NOW(): {now}")
        
        # 2. Check Rate Card '9'
        row = await conn.fetchrow("SELECT id, destination_prefix, effective_start, effective_end FROM rate_cards WHERE destination_prefix = '9'")
        if row:
            print(f"Found Rate Card ID: {row['id']}")
            print(f"Prefix: '{row['destination_prefix']}' (Length: {len(row['destination_prefix'])})")
            print(f"Effective Start: {row['effective_start']}")
            print(f"Effective End: {row['effective_end']}")
            
            # 3. Simulate Query Logic
            prefixes = ['983434724', '98343472', '9834347', '983434', '98343', '9834', '983', '98', '9']
            query = """
                SELECT id, destination_prefix 
                FROM rate_cards
                WHERE destination_prefix = ANY($1)
                  AND effective_start <= NOW()
                  AND (effective_end IS NULL OR effective_end >= NOW())
            """
            result = await conn.fetchrow(query, prefixes)
            print(f"Query Result with prefixes: {result}")
        else:
            print("Row with prefix '9' NOT FOUND via direct select.")

    finally:
        await conn.close()

if __name__ == "__main__":
    asyncio.run(debug_rate())
