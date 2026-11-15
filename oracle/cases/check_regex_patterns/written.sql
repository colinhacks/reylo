insert into codes (id, code, h) values (:id, :code, :h) on conflict (id) do nothing returning code, h
