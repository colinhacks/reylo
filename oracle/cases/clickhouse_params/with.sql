WITH {than:UInt8} AS than SELECT id, age > than AS older, age - than AS by FROM users ORDER BY id, name
