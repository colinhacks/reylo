insert into suppressed (id, name) values (:id, 'x') on conflict (id) do update set name = 'y' returning id
