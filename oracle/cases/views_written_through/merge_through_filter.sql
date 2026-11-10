merge into published p using (select :id::int as id) s on p.id = s.id when not matched then insert (id, title) values (s.id, 'm') returning p.published_at
