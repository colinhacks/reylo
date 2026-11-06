insert into items (id, age, qty) values (:id, :age::int, 5) on conflict (id) do nothing
