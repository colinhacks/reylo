INSERT INTO orders (id, user_id, total, placed) SELECT row.1, row.2, row.3, today() FROM (SELECT arrayJoin({rows:Array(Tuple(UInt64, UInt64, Decimal(18, 2)))}) AS row)
