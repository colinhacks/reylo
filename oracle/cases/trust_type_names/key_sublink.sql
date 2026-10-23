select id from t where id = (select max(x.id) from t x)
