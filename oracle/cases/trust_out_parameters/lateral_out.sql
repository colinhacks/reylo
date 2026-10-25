select sum(r) as s from t, lateral f(t.n) r
