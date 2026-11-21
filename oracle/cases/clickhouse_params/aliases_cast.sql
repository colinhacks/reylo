SELECT id, name FROM users WHERE id = :id::bigint AND name != :not::text
