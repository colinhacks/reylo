do $$ begin
  if (select string_agg(b_id, ',' order by id) from a) is distinct from '1,2' then
    raise exception 'the key column lost its values';
  end if;
end $$;
