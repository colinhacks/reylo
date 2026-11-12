insert into users (id, name) values (:id, :name) on conflict (email) do update set nick = 'x' on conflict do nothing returning id
