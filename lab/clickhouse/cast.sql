SELECT name FROM users WHERE id = :id::UInt32 AND name LIKE :pattern::String LIMIT 1
