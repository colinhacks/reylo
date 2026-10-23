select (doc ->> 'size')::int as size, (doc -> 'owner' ->> 'id')::int as owner_id, upper(doc ->> 'name') as name from docs
