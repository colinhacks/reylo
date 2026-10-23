insert into places (id, location, shape, area) values (:id, :location, :shape, :area) on conflict (id) do nothing returning id, location, shape, area
