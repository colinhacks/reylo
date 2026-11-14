select id from profiles where doc->'$.n' = :v::json
