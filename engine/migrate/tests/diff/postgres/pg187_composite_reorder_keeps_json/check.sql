do $$ begin
  if (select (v).doc::text from t where id = 1) is distinct from '{"k":1,"k":2}' or (select (v).a from t where id = 1) is distinct from 1 or (select v from t where id = 2) is not null then
    raise exception 'the values did not keep their attributes: %', (select array_agg(v::text order by id) from t);
  end if;
end $$;
