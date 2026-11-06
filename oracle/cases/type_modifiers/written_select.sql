insert into typed (id, name, code, price) select :id, :name, :code, :price on conflict (id) do nothing
