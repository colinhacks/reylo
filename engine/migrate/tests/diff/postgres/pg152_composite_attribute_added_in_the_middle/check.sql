do $$ begin
  if (select (x).a from t where id = 1) is distinct from 5 or (select (x).c from t where id = 1) is distinct from 'six' or (select x from t where id = 2) is not null then
    raise exception 'the values did not keep their attributes: %', (select array_agg(x::text order by id) from t);
  end if;
end $$;
