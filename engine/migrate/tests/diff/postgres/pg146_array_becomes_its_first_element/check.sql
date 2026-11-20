do $$ begin
  if (select array_agg(a order by id) from t) is distinct from array[10, 1, 5, null, null]::int[] or (select array_agg(b order by id) from t) is distinct from array[7, 9, 6, null, null]::int[] then
    raise exception 'each array did not become its first element: %, %', (select array_agg(a order by id) from t), (select array_agg(b order by id) from t);
  end if;
end $$;
