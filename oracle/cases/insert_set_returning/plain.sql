insert into p values (:id::int, lower(:name::text)) limit 1 returning id
