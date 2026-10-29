insert into notes (id, body) values (:id, :body) on conflict do update set body = excluded.body where notes.tag is not null returning id
