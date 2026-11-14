insert into items (id, qty, age, price) values (:id, 5, :age, :price) on conflict (id) do nothing
