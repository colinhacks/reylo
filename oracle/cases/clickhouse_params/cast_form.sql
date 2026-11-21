SELECT id, name FROM users WHERE id = :id::UInt64 AND name != :not::String AND created < :before::DateTime64(3, 'UTC') ORDER BY id, name
