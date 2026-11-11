select current_date from (select unnest({'current_date': null::varchar}))
