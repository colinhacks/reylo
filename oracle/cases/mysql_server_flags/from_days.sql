-- FROM_DAYS is NULL past 9999-12-31, which the server's NOT NULL flag does not say.
select id, from_days(n) as d, from_days(3652499) as far from t
