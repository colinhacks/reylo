insert into stamped_parent (id, name) values (:id, 'x') on conflict (id) do update set name = 'y' returning id
