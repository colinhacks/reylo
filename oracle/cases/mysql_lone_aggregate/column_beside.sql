select t.s, json_object('s', t.s, 'a', t.a) as j, count(*) as c from t where t.id = 99
