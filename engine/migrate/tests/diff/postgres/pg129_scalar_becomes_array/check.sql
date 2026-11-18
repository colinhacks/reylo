do $$ begin
  if (select b from t where id = 1) is distinct from array['\x01'::bytea] or (select n from t where id = 1) is distinct from array[1.5] or (select k from t where id = 1) is distinct from array['7']
    or (select n from t where id = 2) is not null or (select k from t where id = 2) is not null then
    raise exception 'a value did not become an array of itself';
  end if;
end $$;
