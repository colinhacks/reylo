insert into p values (unnest(:ids::int[]), 'x') on conflict do nothing returning id
