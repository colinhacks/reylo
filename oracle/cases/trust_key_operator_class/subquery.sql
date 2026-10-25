select t.id from t where t.c = (select x.c from t x where x.id = 1)
