import psycopg2
import os

DATABASE_URL = "postgresql://apolo:apolo123@127.0.0.1:5432/apolobilling"

def fix_rates():
    try:
        print("Connecting to DB...")
        conn = psycopg2.connect(DATABASE_URL)
        cur = conn.cursor()
        
        print("Updating rate_cards...")
        cur.execute("UPDATE rate_cards SET effective_start = '2020-01-01 00:00:00'")
        updated_rows = cur.rowcount
        conn.commit()
        
        print(f"Updated {updated_rows} rows.")
        
        cur.close()
        conn.close()
    except Exception as e:
        print(f"Error: {e}")

if __name__ == "__main__":
    fix_rates()
