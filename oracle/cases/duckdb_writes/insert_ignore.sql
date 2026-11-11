insert into notes (id, body) values (:id, :body) on conflict do nothing returning id
