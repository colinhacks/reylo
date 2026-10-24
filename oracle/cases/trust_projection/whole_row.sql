select length(t) as l, md5(t) as m, abs(t) as a, cardinality(t) as c, now(t) as n, concat_ws(t) as w, uuidv7(t) as u from t
