insert into typed (id, name, code, price, whole, at, at_tz, tenths, flags, tags) values (:id, :name, :code, :price, :whole, :at, :at_tz, :tenths, :flags, :tags) on conflict (id) do nothing
