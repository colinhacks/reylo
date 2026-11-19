INSERT INTO orders SELECT id + {offset:UInt64}, user_id, total, note, placed FROM orders WHERE total > {least:Decimal(18, 2)}
