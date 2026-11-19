select e, count(*) as n from t group by e having e = :p
