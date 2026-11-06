insert into typed (id, name, code, price) values (:id, :name::text, :code, :price::numeric) on conflict (id) do update set price = :new_price
