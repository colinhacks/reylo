select concat(s, count(*)) as x, json_object('s', s, 'n', count(*)) as o, coalesce(s, 'x') as guarded, s is null as missing from t where id = 99
