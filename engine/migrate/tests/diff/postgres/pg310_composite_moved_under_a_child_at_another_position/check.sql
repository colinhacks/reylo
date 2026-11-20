do $$ begin
  if (select array_agg(coalesce(c::text, 'null') order by id) from z) is distinct from array['(1,,x)', 'null', '(,,)'] or (select c::text from r where id = 4) is distinct from '(2,,y)' then
    raise exception 'the values did not keep their attributes: %', (select array_agg(coalesce(c::text, 'null') order by id) from z);
  end if;
end $$;
