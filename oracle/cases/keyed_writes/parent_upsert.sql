insert into parent (id, name) values (:id, 'x') on conflict (id) do update set name = 'skip' returning id
