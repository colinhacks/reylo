insert into stamped_all (id, name) values (:id, 'x') on conflict (id) do update set name = 'y' returning id
